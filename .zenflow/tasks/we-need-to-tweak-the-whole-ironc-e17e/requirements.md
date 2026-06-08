# Product Requirements Document — BrassClaw Home-Use Redesign

**Date:** 2026-05-07  
**Status:** Revised draft — v6  
**Scope:** Full redesign of BrassClaw for local-LLM / home-use deployment

---

## 1. Background & Motivation

BrassClaw was originally built for online LLMs (NEAR AI, Claude, GPT-4o, Gemini Pro) with context windows in the hundreds of thousands of tokens and the ability to handle complex multi-step Python REPL interactions (CodeAct). The current system emits prompt payloads that routinely exceed 10,000–15,000 tokens before a single user message is counted:

- CodeAct preamble alone: ~8.7 KB (≈ 2,165 tokens)
- CodeAct postamble: ~4.0 KB (≈ 1,000 tokens)
- Skill context budget (default): up to 4,000 tokens
- Memory docs injection: up to 5 docs × 500 chars ≈ 625 tokens
- Tool definitions: variable, can add 500–2,000+ tokens

This design is incompatible with consumer-grade hardware running local models through Ollama or an OpenAI-compatible endpoint. Those models typically have **8,192 total context tokens**, and any prompt construction that exceeds that ceiling degrades output quality or causes hard failures.

Additionally, the codebase carries a large legacy "v1" agent execution path alongside the newer engine_v2 design. The two paths are gated by an environment flag and add complexity, dead weight, and maintenance burden.

Finally, every integrated tool, extension, and skill assumes cloud services: Brave Search API, GitHub, Google Workspace, Composio, asana, notion, Stripe, NEAR AI, Slack bots, etc. None of these work without API keys or internet access, making home-use impossible offline or with restricted connectivity.

A further structural problem is that the reactive step loop — where the LLM improvises each step from scratch — is ill-suited to small models. A 7B model running inside an 8,192-token window cannot hold a complex multi-step task in mind across iterations the way a 100B+ cloud model can. Without a pre-execution plan to anchor each step, the local LLM wanders, repeats itself, or produces fragmented incomplete work.

---

## 2. Goals

1. **Single execution model.** Remove the v1 agent entirely. Engine v2 is the only execution path.
2. **Local-LLM-first prompt budget.** Hard cap of **8,192 total prompt tokens** per turn, with skills consuming at most **2,048** of those tokens. The entire prompt-assembly pipeline must respect this ceiling.
3. **Tier 0 execution for local LLMs.** Disable CodeAct (the embedded Python REPL). Local models use only structured tool calls. Cloud backends retain CodeAct as an option.
4. **Local-first integrations.** Replace every online-only tool, extension, and skill with lightweight, locally-runnable equivalents. Cloud integrations become opt-in extras, not defaults.
5. **Missions with an idle-triggered "as routine" mode.** Keep all four learning missions. Add a new cadence mode that fires only when the system has been idle for 10 minutes with no jobs or routines running. Missions in this mode appear in the routines panel like any other routine.
6. **Plan-first execution.** Before the reactive loop begins, the orchestrator must produce or retrieve a compact step-by-step plan. Every execution step is anchored to this plan. Plans are cached and reused across similar future tasks.

---

## 3. Non-Goals

- This is not a complete rewrite of BrassClaw from scratch.
- This does not change the five-primitive engine_v2 data model (Thread, Step, Capability, MemoryDoc, Project).
- This does not remove cloud LLM support. The token budget ceiling applies to all deployments but with user-configurable defaults appropriate to each.
- This does not redesign the channel layer (Telegram, Slack, Discord, etc.) or the gateway/auth layer.

---

## 4. Background Findings

The following findings came out of a codebase audit and inform the requirements below. They establish *what is broken or misaligned today* without prescribing the fix.

### 4.1 Skill token budget is not enforced by engine v2

The web UI Settings → Agents tab already has a "Skill context token size" field that users can change. However, this setting currently controls only the legacy v1 execution path. The engine v2 orchestrator hardcodes the skill token budget internally and never reads the user-facing setting. Once v1 is removed, that UI field will control nothing.

**Impact:** The skill token budget must be properly wired through the engine v2 execution path so that the user-facing UI field works as documented. This is both a correctness fix and a prerequisite for the home-use token guard.

### 4.2 Brave Search is the only built-in web access mechanism

There is no local fallback for web search. Removing the Brave API dependency leaves the agent with no way to access current web content unless replaced.

### 4.3 CodeAct prompt is the dominant token consumer

The CodeAct system prompt alone accounts for roughly 3,000–3,200 tokens. Even before skills, memory docs, or tool schemas are injected, local models operating at 8,192 tokens are already left with fewer than 5,000 tokens for content and conversation. CodeAct must be suppressed for local model sessions.

### 4.4 DocType::Plan exists but is unused

The engine v2 memory type system already defines a `Plan` document type with a retrieval weight assigned. No code in the orchestrator creates or retrieves plan documents. The type was anticipated but never implemented.

**Impact:** Plan-first execution can be built on top of the existing memory infrastructure without changing the data model.

### 4.5 Orchestrator can make isolated LLM calls

The compaction path in the orchestrator already calls the LLM with a custom minimal message list, bypassing the full system prompt and conversation history. This same mechanism is the foundation for the planning call (REQ-6.4).

---

## 5. Detailed Requirements

### 5.1 Remove the v1 Agent Execution Path

**REQ-1.1** All code whose sole purpose is to support the v1 agent execution path must be removed. This includes the v1 session, job, routine, and dispatcher logic. The Technical Specification step is responsible for producing the exact file-by-file audit.

**REQ-1.2** The environment flag used to switch between v1 and v2 must be removed. Engine v2 is unconditionally active; no fallback to v1 exists after this change.

**REQ-1.3** Skill-related helper code that exists only to support the v1 agent (trust-based tool attenuation, v1-specific credential conversion helpers) must be removed alongside the v1 agent. The core skills library (types, parser, selector, validation) is retained.

**REQ-1.4** Online-only tool and MCP-server registry entries that have no local equivalent must be removed from the default registry. Integrations that work with self-hosted or locally-operated services (e.g. Nextcloud-compatible services, self-hosted task trackers) may be retained as opt-in extras. The Technical Specification step is responsible for the exact per-entry audit.

**REQ-1.5** The online skill catalog (cloud-based registry discovery) must be removed as a runtime dependency. Skills are installed locally or shipped bundled with the application.

**Post-condition:** After the v1 removal, the system must compile, pass its test suite, and be fully functional using only the engine v2 path.

---

### 5.2 Token Budget System (Token Guard)

**REQ-2.1 — Configurable total prompt token ceiling.**
The system must enforce a hard ceiling on the total number of tokens assembled into each LLM prompt turn. This ceiling must be:
- User-configurable at runtime (no restart required) via the settings UI
- Applicable across all prompt content: system prompt, skills, memory docs, tool schemas, and conversation history
- Defaulted to **8,192 tokens** for local/home-use profiles and to a much larger value for server/cloud profiles

**REQ-2.2 — Skill token sub-budget, correctly wired.**
The existing user-facing "Skill context token size" setting must be correctly enforced by the engine v2 execution path (see finding 4.1 — it currently is not). Its default must be lowered to **2,048 tokens** for local/home-use profiles. No new UI field is needed for this; the existing field is retained with its existing DB key. Its description must be updated to clarify that this value is a sub-budget within the total ceiling from REQ-2.1.

**REQ-2.3 — New "Max total prompt tokens" UI field.**
A new user-facing numeric field ("Max total prompt tokens") must be added to the Settings → Agents tab, directly below the existing "Skill context token size" field. It controls the total ceiling from REQ-2.1. Saving a value smaller than the current skill token sub-budget must be rejected with a clear, user-facing error message.

**REQ-2.4 — Graceful degradation when budget is exceeded.**
When the assembled prompt would exceed the ceiling, the system must first attempt to reduce content by dropping lower-priority items in the following order:

1. Memory docs — drop lowest-scoring first (summaries and notes before lessons)
2. Skills — drop lowest-scoring first
3. Tool schema verbosity — truncate action descriptions while preserving parameter names and types
4. System prompt non-essential sections (examples, postamble)
5. Conversation history — drop oldest messages first (never drop the most recent user message or the active plan anchor)

The active execution plan (REQ-6) must **never** be dropped by this mechanism — it is treated as part of the system prompt reservation and is not subject to degradation.

If the prompt still exceeds the ceiling after all of the above have been applied — meaning the task itself is too large for the budget — the system must trigger **task decomposition** (REQ-6.9) rather than failing, truncating the goal, or discarding essential content silently. This decomposition trigger does not apply when the overflowing prompt belongs to a subtask that was itself produced by a prior decomposition step; in that case REQ-6.9.4 governs and the subtask fails with a clear user-facing error.

The user must not need to take any action for either the dropping or the decomposition to occur.

**REQ-2.5 — Budget applies to both home-use and cloud profiles.**
The token ceiling is not exclusive to local model setups. Cloud-profile users can also configure it if desired. The defaults simply differ between profiles.

**REQ-2.6 — Planning calls are outside the main token guard.**
The minimal planning LLM call (REQ-6.4) is a separate, isolated call with its own hard budget cap (≤ 200 tokens total input). It does not consume from the main 8,192-token conversation budget. The token guard must not apply its degradation logic to planning calls.

**Note on conversation headroom:** With the local default of 8,192 total tokens, and reserving ~1,024 for the system prompt (which includes the active plan anchor), 2,048 for skills, 512 for memory docs, and 1,024 for tool schemas, approximately 3,584 tokens remain for conversation history and the model's response. At typical turn lengths this covers roughly 7–10 exchanges. Longer conversations will experience oldest-message truncation. This is an intentional trade-off of the home-use profile and should be documented in the user-facing configuration description.

---

### 5.3 Tier 0 (Structured Tool Calls) for Local LLMs

**REQ-3.1 — Suppression of CodeAct for local backends.**
When the configured LLM backend is a locally-run model (Ollama or any OpenAI-compatible endpoint), the system must not emit the CodeAct system prompt or attempt to execute embedded Python REPL steps. All tool interactions use structured tool calls only.

**REQ-3.2 — Compact system prompt for Tier 0 sessions.**
When CodeAct is suppressed, the system prompt must be replaced with a compact alternative. The total system prompt — including the platform identity section, the tool-calling instructions, and the active plan anchor (REQ-6) — must fit within 1,024 tokens. The prompt must be written in plain language suitable for smaller models (7B–13B range): short sentences, no jargon, no Python examples. It must explain the agent's identity, the structured tool-calling pattern, how a turn ends, and include a dedicated section that shows the active plan and the current step position.

**REQ-3.3 — User override.**
Advanced users must be able to re-enable CodeAct for a local backend via a configuration setting. This supports users running large local models (e.g. large Qwen or Llama variants) that can handle CodeAct.

**REQ-3.4 — Cloud backends unaffected.**
Cloud LLM backends (Anthropic, OpenAI, Gemini, NEAR AI) retain CodeAct by default. The existing CodeAct prompt files are not modified.

**REQ-3.5 — Planning calls are always text-only.**
The planning call (REQ-6.4) must never use CodeAct, regardless of backend type or user override settings. The planning call uses the isolated minimal-message path and always requests a text response.

> Plan invalidation behaviour when the user changes the task goal mid-thread is specified in REQ-6.8.

---

### 5.4 Local-First Integrations

#### 5.4.1 Built-in Local Web Browsing

**REQ-4.1 — Replace cloud web search with local browser tool.**
The Brave Search API integration must be replaced with a local browser-based web tool that requires no API key and no cloud account. The tool must be auto-launched by BrassClaw (not require the user to start a separate process), gracefully detect whether its runtime dependency is installed, and provide a clear install hint if it is not.

**REQ-4.2 — Browser tool capabilities.**
The local browser tool must expose, at minimum: navigate to a URL, search the web by keyword, extract page text, take a screenshot, click an element, and fill a form field.

**REQ-4.3 — Browser tool sandboxing.**
The local browser tool must not have access to the local file system. It may access localhost (for development workflows) and public HTTP/HTTPS.

**REQ-4.4 — Web-browsing skill.**
A skill that activates on web search / current events / URL queries must accompany the browser tool. This skill's total injected context must not exceed 256 tokens.

#### 5.4.2 Skill and Tool Schema Trimming

**REQ-4.5 — All built-in skills must declare a token budget.**
Every built-in skill must declare its maximum context token consumption in its manifest. Skills that do not declare a budget must be excluded from injection entirely (budget = 0). This is a **breaking change** from the current behavior where a missing declaration falls back to a default of 2,000 tokens.

> **Migration risk — REQ-4.5:** Any existing skill (bundled or user-installed) that lacks a declared budget will silently stop being injected after this change. The system must mitigate this as follows:
> 1. At startup, log a named warning for each discovered skill that lacks a declared budget, identifying it by name and path.
> 2. For **bundled skills** (shipped with the application), the team is responsible for adding the declaration before release; no skill may ship without one.
> 3. For **user-installed skills**, the silent-exclusion behavior applies immediately, but the startup warning gives users visibility. A future grace period or a default of a small non-zero value (e.g. 256 tokens) may be introduced in a subsequent release.

**REQ-4.6 — Built-in skill size caps.**
All built-in skills must be rewritten or trimmed to fit within their declared budget. The following caps apply:
- Skills that provide brief behavioral guidance (commit style, idea capture, decision recording): ≤ 128 tokens declared budget
- Skills that provide tool usage instructions or domain procedures (coding, web browsing, local dev): ≤ 256 tokens declared budget
- Skills that depend on cloud API credentials not present in the environment must be gated: they may not activate if the required credential is absent

**REQ-4.7 — Tool schema verbosity reduction and count constraint.**
All built-in tool action schemas exposed to the model at runtime must be trimmed. Action descriptions may not exceed 60 words. Long usage examples and markdown docstrings must not appear in the runtime-facing schema. Developer documentation may remain in source code or a separate reference file.

The 1,024-token tool schema reservation (from the local default budget) accommodates at most 8–12 tools at ~80–120 tokens each. Every new built-in tool added by this project (browser, local search, CalDAV, notes) must be accompanied by an audit confirming the total tool list still fits within budget. Tools that are seldom used must be considered for on-demand registration (loaded only when a matching skill is active) rather than always-present registration.

**REQ-4.8 — Online-only skill removal.**
Skills that exist solely to configure or operate cloud-only services (and that cannot function at all without a cloud API key or account) must be removed from the default bundled set. Skills for services that have self-hosted equivalents may be retained as opt-in extras, gated on the required credential being configured.

#### 5.4.3 New Local Tools

**REQ-4.9 — Local file search tool.**
A built-in action for full-text search over local workspace files must be provided. The search scope must default to the currently active project workspace directory. Users may configure additional search paths beyond the default. Whole-filesystem search must never be the default; it may be offered as an explicit opt-in. The tool must not require any external process or API key.

**REQ-4.10 — CalDAV calendar and to-do tool.**
A built-in calendar and to-do integration must be provided using the CalDAV standard (RFC 4791). This protocol is supported by popular self-hosted calendar servers (Nextcloud, Baikal, Radicale, ownCloud, Synology Calendar) as well as iCloud (via app-specific passwords), following the same integration pattern as Home Assistant's CalDAV integration. The tool must:
- Accept a server URL, username, and optional password stored in the BrassClaw secrets vault (never in plain config)
- Auto-discover available calendars from the server without requiring a manual list
- Support an optional filter to restrict which calendars are surfaced
- Support a flag to disable SSL verification for self-signed certificates common on home servers
- Expose separate actions for calendar events and to-do lists
- Maintain a local cache of the most recently fetched calendar data, refreshed on a background interval (default ~15 minutes); agent tool calls read from this cache and do not block on a live network fetch, ensuring fast responses even if the CalDAV server is temporarily unavailable

**REQ-4.11 — Local notes tool.**
A built-in action for appending to and reading from a structured notes file (Markdown format) must be provided. The notes file is **global** (not per-project) by default, reflecting home-use patterns where a single user accumulates notes across all contexts. The storage path must be user-configurable. The tool must require no external service or API key.

#### 5.4.4 Home Assistant Integration Reference

Home Assistant's integration catalog ([https://www.home-assistant.io/integrations/](https://www.home-assistant.io/integrations/)) is the reference design for home-use local integrations. The following HA patterns identify future integration candidates beyond this release. The architecture must not preclude adding them:

| HA Pattern | Relevance to BrassClaw |
|---|---|
| CalDAV | In scope — REQ-4.10 |
| REST / webhook generic HTTP | Already possible via browser tool |
| MQTT message broker | Future: event-driven local automation |
| Folder watcher | Future: workspace change awareness |
| Shell command | Already available via shell tool |
| Notification dispatch | Future: maps to existing SSE infrastructure |

---

### 5.5 Mission System — Idle-Triggered "As Routine" Mode

**REQ-5.1 — New idle cadence mode.**
The mission system must support a new cadence mode alongside the existing event-driven and cron modes. In this mode ("As Routine / Idle"), a mission fires only when all of the following are true simultaneously:
- The system has received no user message and completed no thread for at least the configured idle threshold
- No thread is currently running or waiting
- No other routine or job is pending or running

The idle threshold is **per-mission** — each mission in idle mode has its own configurable threshold, defaulting to 10 minutes if not set.

**REQ-5.2 — Idle missions appear in the routines panel.**
A mission running in idle mode must appear in the routines panel (both the TUI and the web UI) as a routine entry. While waiting for idle conditions it shows a "Waiting (idle)" status. While its thread is active it shows "Running". After completion it shows the same completion state as any other routine.

**REQ-5.3 — At most one idle mission fires per idle window.**
When multiple missions are configured for idle-triggered execution, at most one fires per idle window. When two or more missions are simultaneously eligible, the one that appears first in the missions configuration list fires first (declaration order). Once a mission thread starts, the idle clock resets and other idle-configured missions must wait for the next idle period.

**REQ-5.4 — Configuration per mission.**
Each of the four built-in learning missions (self-improvement, skill-repair, skill-extraction, conversation-insights) must independently support being configured as: event-triggered (current behavior), idle-triggered (new), or disabled. The default remains event-triggered to avoid breaking existing server deployments.

**REQ-5.5 — UI toggle.**
The missions configuration UI (web and TUI) must expose the idle mode as a selectable cadence option. When idle mode is selected for a mission, the per-mission idle threshold must be displayed and editable alongside it.

**REQ-5.6 — Idle timer must survive restarts.**
The "last activity" timestamp used to determine whether the idle threshold has been reached must be persisted to durable storage, not held only in memory. A restart must not reset the idle clock. This ensures that missions in idle mode fire reliably even for users who restart BrassClaw frequently.

---

### 5.6 Plan-First Execution

**REQ-6.1 — Plan-first execution for non-trivial tasks.**
Before the reactive tool-call loop begins, the orchestrator must produce or retrieve a compact execution plan for the current task. The plan is a terse numbered step list with no explanatory prose, no justifications, and no filler text. Every execution step runs anchored to this plan: the LLM always knows its current step position and the remaining steps. The active plan is always included in the context passed to the LLM and is never dropped by the token guard (REQ-2.4).

**REQ-6.2 — Trivial task bypass.**
The planning phase must be skipped for tasks that can reasonably be resolved in a single tool call or a single text response. The heuristic for detecting trivial tasks must cover at minimum: questions (goal contains a question mark with no multi-step structure), very short goals (word count below a configurable threshold, default 8 words), and goals that match a known single-step pattern (e.g., "what time is it", "add X to notes"). The bypass must err on the side of planning rather than skipping — false positives (planning when unnecessary) are cheaper than false negatives (not planning when needed).

**REQ-6.3 — Bundled plan templates.**
A library of plan templates covering the most common home-use task patterns must be shipped with the application. Templates are stored as `DocType::Plan` MemoryDocs and loaded at startup alongside skills. They are matched to incoming goals using the same keyword and tag scoring used for skill selection. When a goal matches a template above a confidence threshold, no planning LLM call is made — the matched template is used directly. The bundled template library must cover at minimum the following task categories:
- Install or remove a software package
- Web search and summarize result
- Read or write calendar events
- Append to or search notes
- Search workspace files for a pattern
- Git operations (status, commit, push)
- Check system information

**REQ-6.4 — Minimal-context runtime planning call.**
When no suitable template is found (REQ-6.3) and no high-confidence cached plan matches (REQ-6.5), the orchestrator must produce a plan via a dedicated LLM call with the following strict constraints:
- Total input must not exceed 200 tokens
- The system message must be at most 20 tokens: a single sentence instructing the model to output only a numbered step list with no preamble
- The user message must contain: the task goal and the names of available tools (not their schemas or descriptions)
- No skills, memory docs, conversation history, tool schemas, or prompt preamble may be injected
- The call must always request a text response (never CodeAct), regardless of the configured backend or user override
- The response parser must extract the numbered list and strip any preamble that the model produces before the first numbered item
- If the goal alone — with no tool names included — still exceeds the available budget after the system message, the goal is too complex for a direct planning call. In this case the system must trigger **task decomposition** (REQ-6.9) instead of truncating or skipping

**REQ-6.5 — Plan caching and reuse.**
Runtime-generated plans must be stored as `DocType::Plan` MemoryDocs with keywords and tags extracted from the goal at planning time. On future tasks where goal keywords match a cached plan above the confidence threshold, the cached plan is reused instead of making a new planning call. Cached plans participate in the standard memory retrieval scoring and are surfaced at step 0 of the orchestrator alongside lessons and summaries.

**REQ-6.6 — Plan confidence tracking.**
Each plan document tracks a confidence score using the same mechanism as skill confidence tracking. A successful full execution of the plan raises confidence. A failed, stalled, or abandoned execution lowers it. Plans whose confidence falls below a configurable threshold are treated as invalid and trigger a new planning call rather than reuse. This prevents bad plans from propagating to future tasks.

**REQ-6.7 — Plan anchor injection.**
At every step of the execution loop (not only step 0), the LLM's context must include: the full plan step list, the index of the current step, and the count of remaining steps. This anchor is injected as a compact section within the system prompt reservation (REQ-3.2). The anchor must be formatted to consume at most 200 tokens regardless of plan length; plans longer than this limit must be summarized to fit.

**REQ-6.8 — Plan invalidation on goal change.**
When the orchestrator detects that the user has injected a new message that materially changes the task goal (new explicit task instruction, contradicting the current plan direction), it must discard the current plan and start a new planning cycle before the next execution step. The detection heuristic must not trigger on clarifications, confirmations, or supplemental information that is consistent with the current plan.

**REQ-6.9 — Task decomposition on budget overflow.**
Task decomposition is triggered in two conditions: (a) the planning call goal overflows its own budget after removing all tool names (REQ-6.4), or (b) the main prompt still exceeds the total ceiling after full graceful degradation (REQ-2.4). In both cases the system must:

1. **Produce a miniplan** via an isolated LLM call containing exactly two parts: a system message of at most 15 tokens ("Break task into subtasks. One per line. Brief.") and the original task goal verbatim. No tools, no skills, no context, no conversation history. The goal must not be truncated — the miniplan's purpose is to decompose goals that are too large or complex for direct planning, so receiving the full goal is essential. The output is 2–4 single-line subtask goals. If the miniplan output is empty or incoherent (detectable by the absence of any numbered or line-separated items), the system fails with a clear user-facing error rather than proceeding with broken subtasks.

2. **Execute subtasks sequentially** through the full plan-first pipeline — each subtask goes through template matching (REQ-6.3), its own planning call (REQ-6.4), and its own execution loop against the same token budget. Each subtask runs with a fresh context window, avoiding accumulated context bloat across subtasks.

3. **Thread results forward** — the output summary of each completed subtask is injected as a compact context note at the start of the next subtask's execution. The summary must not exceed 200 tokens. The final subtask's output becomes the result of the original task.

4. **Limit decomposition depth to one level** — subtasks must not themselves trigger further decomposition. If a subtask still exceeds budget after full degradation, it fails with a clear error rather than spawning sub-subtasks. This prevents unbounded recursion in this release.

5. **Cache the decomposition** — a successful miniplan that produced working subtasks is cached as a `DocType::Plan` MemoryDoc with the original goal's keywords, so future similar large tasks reuse the decomposition without a miniplan call.

---

## 6. Constraints & Risks

| # | Item |
|---|---|
| C1 | All changes must keep the engine v2 five-primitive data model intact. |
| C2 | The engine v2 crate must remain independent from the main application crate (no circular dependency introduced). |
| C3 | Token counting uses the existing character-based approximation throughout this release. A proper tokenizer integration is a future concern. |
| C4 | The local browser tool's runtime (Node.js + Playwright) is an optional dependency. Its absence must not prevent BrassClaw from starting or functioning; only browser-dependent features degrade gracefully. |
| C5 | v1 code removal can be staged across multiple commits provided the system compiles and CI passes at each stage. |
| C6 | CodeAct remains available for cloud LLM users. No functionality is removed from the cloud path. |
| C7 | The 1,024-token tool schema budget constrains the number of always-present tools to approximately 8–12. Every tool added requires an audit of the total tool list against this ceiling. On-demand tool registration (activated only when a matching skill is active) must be the pattern for tools that are not needed on every turn. |
| C8 | The planning call's 200-token input cap is a hard constraint. The miniplan call has no fixed total budget because it must receive the original goal verbatim; its only constraint is the 15-token system message. Plan templates, caching, and decomposition caching exist precisely to avoid hitting these paths repeatedly; runtime planning and miniplan calls are last resorts, not routine paths. |
| C9 | Task decomposition is capped at one level of depth in this release. A subtask that still exceeds budget after full degradation fails with a clear error; it does not recurse further. |
| R1 | **Breaking change — skill budget declaration (REQ-4.5).** Existing user-installed skills without a declared token budget will stop being injected. Startup warnings are mandatory. See REQ-4.5 migration section. |
| R2 | **Conversation headroom trade-off.** The local default token budget leaves approximately 3,500 tokens for conversation history. Long conversations will experience oldest-message truncation. This must be documented clearly in the UI description for the total token ceiling field. |
| R3 | **Plan quality risk for novel tasks.** A 7B local model may produce a low-quality plan for tasks outside its training distribution. The confidence tracking mechanism (REQ-6.6) mitigates this over time, but the first execution of a genuinely novel task may be suboptimal. Users should be aware that plan quality improves with usage. |
| R4 | **CalDAV credential security.** CalDAV credentials (server URL, username, password) must be stored in the BrassClaw secrets vault with the same encryption guarantees as all other credentials. Misconfigured self-signed certificate handling (the `verify_ssl: false` flag) must be surfaced as a warning to the user at configuration time. |
| A1 | "Home use" means single-user, single-machine deployment. Multi-tenant concerns are secondary. |
| A2 | Local web browsing via Playwright covers the majority of web-search use cases (news, documentation, product info). Search throughput and volume do not need to match a cloud search API. |
| A3 | The idle mission mode is opt-in. Existing server deployments are unaffected unless they explicitly switch a mission to idle mode. |
| A4 | Skills depending on cloud APIs (GitHub, Linear, Notion) remain as installable opt-ins; they simply must not activate by default when no credential is configured. |
| A5 | The planning call assumes that the local LLM can follow a simple numbered-list instruction. Models below ~3B parameters may not reliably comply; this is considered out of scope for the local minimum hardware target. |
| C10 | **Mission thread config propagation (deferred).** Mission threads currently use `ThreadConfig::default()` instead of operator-configured settings. This means `max_prompt_tokens` and other tuned fields are not respected for mission-spawned threads. Tracked as Step 19. |
| C11 | **`count_running_threads` O(N) complexity (deferred).** The running-thread gate in `fire_mission` iterates all thread history entries with per-entry store lookups. Acceptable for current workloads; will become a bottleneck for long-lived missions with hundreds of threads. Tracked as Step 20. |
| C12 | **Plan doc deduplication (deferred).** `__save_plan_doc__` creates a fresh `MemoryDoc` per call without checking for existing docs with the same goal prefix. This produces duplicate cache entries that waste token budget during retrieval. Not a correctness bug — highest-confidence doc wins — but an efficiency concern. Tracked as Step 21. |
| C13 | **CJK/multilingual token under-estimation (deferred).** The byte-based approximation (`bytes × 0.25`) is conservative for ASCII but under-estimates CJK, Arabic, and emoji text by 50–100%. A proper tokenizer or adjusted multiplier is needed for multilingual home-use scenarios. Tracked as Step 22. |

---

## 7. Success Metrics

| Metric | Target |
|---|---|
| Total prompt tokens for a fresh single-skill conversation turn | ≤ 4,000 tokens |
| Total prompt tokens at max load (budget-limited skills + memory + tools) | ≤ 8,192 tokens (guard enforces this) |
| System prompt size for a local/Tier 0 session (including plan anchor) | ≤ 1,024 tokens |
| Each bundled skill's declared token budget | ≤ 256 tokens |
| Planning call total input size | ≤ 200 tokens |
| Plan anchor injected per step | ≤ 200 tokens |
| v1-only code remaining after cleanup | 0 (verified by CI) |
| v1/v2 feature-flag references remaining after cleanup | 0 |
| Local web browsing working without any API key | Yes |
| Idle-triggered mission visible in routines panel | Yes |
| Idle timer survives a process restart | Yes |
| Startup warning emitted for skills missing budget declaration | Yes |
| Bundled plan templates covering common home-use tasks | ≥ 7 categories (REQ-6.3) |
| Novel task gets a cached plan after first successful execution | Yes |
| Miniplan system message | ≤ 15 tokens; goal passed verbatim, not truncated |
| Task decomposition triggered on budget overflow rather than silent truncation | Yes |
| Decomposition depth cap enforced | 1 level |
| Successful decomposition cached for future reuse | Yes |
