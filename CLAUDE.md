# BrassClaw Development Guide

Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

Tradeoff: These guidelines bias toward caution over speed. For trivial tasks, use judgment.

1. Think Before Coding

Don't assume. Don't hide confusion. Surface tradeoffs.

Before implementing:

State your assumptions explicitly. If uncertain, ask.
If multiple interpretations exist, present them - don't pick silently.
If a simpler approach exists, say so. Push back when warranted.
If something is unclear, stop. Name what's confusing. Ask.
2. Simplicity First

Minimum code that solves the problem. Nothing speculative.

No features beyond what was asked.
No abstractions for single-use code.
No "flexibility" or "configurability" that wasn't requested.
No error handling for impossible scenarios.
If you write 200 lines and it could be 50, rewrite it.
Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

3. Surgical Changes

Touch only what you must. Clean up only your own mess.

When editing existing code:

Don't "improve" adjacent code, comments, or formatting.
Don't refactor things that aren't broken.
Match existing style, even if you'd do it differently.
If you notice unrelated dead code, mention it - don't delete it.
When your changes create orphans:

Remove imports/variables/functions that YOUR changes made unused.
Don't remove pre-existing dead code unless asked.
The test: Every changed line should trace directly to the user's request.

4. Goal-Driven Execution

Define success criteria. Loop until verified.

Transform tasks into verifiable goals:

"Add validation" → "Write tests for invalid inputs, then make them pass"
"Fix the bug" → "Write a test that reproduces it, then make it pass"
"Refactor X" → "Ensure tests pass before and after"
For multi-step tasks, state a brief plan:

1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.


**BrassClaw** is a secure, local-first AI assistant built on the IronClaw Reborn architecture. It targets 7B-14B LLMs within 8,192-token context windows and is implemented as a workspace of approximately 70 Rust crates.

## Build and Test

> **Mandatory:** All `cargo build`, `cargo test`, `cargo clippy`, and `cargo check` invocations
> **must** be launched in a `screen` session so the terminal is not blocked and implementation
> can continue while compilation runs.
>
> ```bash
> screen -dmS <name> bash -c 'cd /Volumes/SSDE/brassclaw && <cargo command> 2>&1 | tee /tmp/<name>.log; echo "EXIT:$?" >> /tmp/<name>.log'
> # Check progress / result at any time:
> tail -f /tmp/<name>.log
> grep "^EXIT:" /tmp/<name>.log   # non-empty = finished
> ```

> **Mandatory:** Every `cargo build`/`test`/`clippy`/`check` **must** set
> `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target` (NVMe) — never build in-place on the
> slow external repo drive. **Before** compiling, check free space on that volume and clean
> it if it is too full:
>
> ```bash
> df -h /Users/ollama/brassclaw-target          # check before every compile
> # If Avail < 15 GB or Capacity > 90%, clean first:
> CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo clean
> # Then run the actual command with the target dir set:
> CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo <build|test|clippy|check> ...
> ```
>
> The NVMe target dir accumulates multi-GB artifacts and can fill the 228 GB volume
> mid-build, starving/corrupting the run — the space check + clean is mandatory, not optional.

```bash
cargo fmt                                                              # format
cargo clippy --all --benches --tests --examples --all-features         # lint (zero warnings)
cargo test                                                             # unit tests
cargo test --features integration                                      # + PostgreSQL tests

# Build the Reborn binary with WebUI v2
cargo build --release --bin brassclaw

# Run with logging
BRASSCLAW_REBORN_LOG=brassclaw=debug cargo run
```

### Avoid redundant rebuilds

`cargo build` on this workspace is slow. Capture output once and inspect it multiple times — do **not** rerun the build just to see different output:

```bash
# Capture and display simultaneously
cargo build --release --bin brassclaw 2>&1 | tee build.log

# Analyse the saved log without rebuilding
grep "^error" build.log
grep -n "warning\|error" build.log | head -40
cat build.log | less
```

Before invoking `cargo build` a second time, check whether `build.log` (or any previously captured log) already contains the information needed.

E2E tests: see `tests/e2e/CLAUDE.md`.

## Testing Configuration

### LLM Configuration for Tests

When running Playwright tests or manual testing, use the following LLM configuration:

**OpenAI-Compatible Provider:**
- **Name:** Qwen-Test (or any name)
- **Type:** openai-compatible
- **Base URL:** http://192.168.10.223:8000/v1
- **Model:** Qwen/Qwen2.5-7B-Instruct-AWQ
- **API Key:** None required (leave empty)

**Gateway Token:**
```bash
export BRASSCLAW_GATEWAY_TOKEN=your-token-here
```

This token is required for authentication with the brassclaw server during testing. Set it to your actual gateway token value.

### Quick Test Setup

```bash
# Set gateway token
export BRASSCLAW_GATEWAY_TOKEN=your-token-here

# Start server
cd /Volumes/SSDE/brassclaw
cargo run --release -- serve --host 127.0.0.1 --port 3000

# In another terminal, run tests
cd /Volumes/SSDE/brassclaw/tests/playwright-agent
npm test
```

### Manual Testing via WebUI

1. Start the server with gateway token:
   ```bash
   export BRASSCLAW_GATEWAY_TOKEN=your-token-here
   cargo run --release -- serve --host 127.0.0.1 --port 3000
   ```

2. Open browser to http://127.0.0.1:3000

3. Configure LLM provider:
   - Go to Settings → Providers
   - Add new provider with above configuration
   - Test connection

4. Start chatting with the agent

### Playwright Agent Tests

The Playwright test suite in `tests/playwright-agent/` includes:
- **01-connection.spec.ts** - Connection and authentication tests
- **02-llm-config.spec.ts** - LLM configuration tests
- **03-agent-interaction.spec.ts** - Agent interaction and conversation tests

See `tests/playwright-agent/README.md` for detailed test documentation.

## Creating Releases

### Automated Release Process

This project uses GitHub Actions for automated releases. **Do not build binaries manually.**

### How to Create a Release

Simply push a version tag:

```bash
git tag v0.29.9
git push origin v0.29.9
```

GitHub Actions will automatically:
1. Build binaries for all platforms (Linux x86_64, macOS ARM64, macOS x86_64)
2. Generate SHA256 checksums for each binary
3. Create a GitHub release with auto-generated release notes
4. Upload all artifacts (binaries + checksums) to the release

### Monitoring Builds

- **Workflow runs**: https://github.com/chtugha/brassclaw/actions/workflows/release.yml
- **All actions**: https://github.com/chtugha/brassclaw/actions

### Supported Platforms

- **Linux x86_64**: `x86_64-unknown-linux-musl` (statically linked)
- **macOS ARM64**: `aarch64-apple-darwin` (Apple Silicon)
- **macOS x86_64**: `x86_64-apple-darwin` (Intel)

### Release Workflow Details

See `CICD_SETUP_DOCUMENTATION.md` for comprehensive documentation on:
- Workflow architecture
- Build process details
- Testing procedures
- Troubleshooting guide
- Maintenance instructions

### Important Notes

- **Never manually build and upload binaries** - always use the automated workflow
- **Tag format**: Use semantic versioning with `v` prefix (e.g., `v0.29.9`, `v1.0.0`)
- **Build time**: Expect 10-15 minutes for all platforms to build
- **Artifacts**: Each release includes 6 files (3 binaries + 3 checksums)

## Code Style

- Prefer `crate::` for cross-module imports; `super::` is fine in tests and intra-module refs
- No `pub use` re-exports unless exposing to downstream consumers
- No `.unwrap()` or `.expect()` in production code (tests are fine)
- Use `thiserror` for error types in `error.rs`
- Map errors with context: `.map_err(|e| SomeError::Variant { reason: e.to_string() })?`
- Prefer strong types over strings (enums, newtypes)
- Keep functions focused, extract helpers when logic is reused
- Comments for non-obvious logic only
- Multi-line prompt strings (mission goals, system prompts, CodeAct preambles) go in `crates/brassclaw_engine/prompts/*.md` and are loaded via `include_str!()`. Never inline large prompt templates as Rust string constants — they are hard to read, review, and iterate on. Single-line format strings are fine inline.
- `info!` and `warn!` output appears in the REPL and corrupts the terminal UI. Use `debug!` for internal diagnostics (trace analysis, reflection results, engine internals). Reserve `info!` for user-facing status that the REPL intentionally renders. Background tasks must never use `info!`.
- Test through the caller, not just the helper: when a predicate/classifier/transform helper gates a side effect (HTTP, DB write, OAuth, UI mutation, tool execution) and has any wrapper or computed input between it and that side effect, a unit test on the helper alone is not sufficient regression coverage. Add a test that drives the call site at the integration tier or higher. See `.claude/rules/testing.md` for the full rule.

## Tool Usage Guidelines

### Ripgrep (rg) Searches

When performing ripgrep searches across the codebase, delegate these tasks to the large-file-reading mode instead of executing them directly. This prevents process termination issues and ensures proper handling of large search results.

Example delegation:
- Task: Search for patterns across codebase
- Mode: large-file-reading
- Reason: Handles large output and prevents SIGKILL issues


## Architecture

BrassClaw Reborn uses a four-layer model:

1. **Products** — UX surfaces and deployment shapes (CLI, web server, daemon). Products wire together loops, capabilities, and host access. They do not implement agent logic.
2. **Loops** — Agent behavior drivers. A loop manages planning, tool dispatch, turn sequencing, approval gates, checkpointing, retries, and completion. All agentic execution passes through the loop runner.
3. **Kernel** — Authority and policy enforcement. Trust decisions, secret resolution, safety policy, sandboxing, capability grants, and session identity live here. Kernel boundaries are enforced; product and loop code cannot override them.
4. **Infrastructure** — Shared services: LLM providers, Postgres persistence, embeddings, skills, extensions, and observability.

New work belongs in `crates/`. The v1 `src/` tree was removed in Phase 6.

### Component Catalog and Class Codes

BrassClaw Reborn stores all reusable knowledge artifacts (specs, plans, lessons, etc.) in unified Postgres tables indexed by integer **class codes**. Each class has a dedicated table:

| Class code | Type | Table |
|------------|------|-------|
| 0 | Tool | `reborn_tools` |
| 1 | Leaf Skill (Rusty) | `reborn_skills` |
| 2 | Domain Skill (Monty) | `reborn_skills` |
| 10 | Orchestrator | `reborn_component_catalog` (class 10) |
| 11 | Actions | `reborn_actions` |
| 12 | Spec | `reborn_specs` |
| 13 | ToolSkill | `reborn_tool_skills` |
| 14 | Plan | `reborn_plans` |
| 15 | Summary | `reborn_summaries` |
| 18 | Lesson | `reborn_lessons` |
| 19 | Issue | `reborn_issues` |
| 20 | Note | `reborn_notes` |
| 21 | Recipe | `reborn_recipes` |
| 22 | PythonCode | `reborn_python_code` |
| 23 | ExtensionCatalogue | `reborn_extension_catalogues` |
| 50 | Scaffold | `reborn_component_catalog` (class 50) |

Legacy `brassclaw_memory_docs` rows are migrated into the appropriate class table at boot by `run_component_import` (`crates/brassclaw_reborn_composition/src/component_import.rs`).

### Orchestrator-First, LLM-Minimal (Core Design Principle)

**Monty (the Python orchestrator) IS the execution engine and the sole
execution authority.** Rust makes tools *available*; an LLM never executes
anything itself — it only writes Python that Monty runs in the sandbox. The LLM
is consulted only when creative reasoning, content composition, or user
confirmation is genuinely required.

**Execution model — Orchestrator + Executioner (locked 2026-09-02):**
BrassClaw has an **Orchestrator** and an **Executioner**.

- **Orchestrator (Monty, Python)** is the brain and the sole execution
  authority. It runs **one long-persisting main process per user input**
  (start → intent → match/no-match → answer → history → exit), recipe/
  intent-driven. It reads Recipes/Instructions, sequences steps, assembles
  every LLM prompt, and **calls tools by name**. It never executes Rust
  directly — it calls tools.
- **Executioner (Rust)** is the muscle. It holds **precompiled Tools +
  ToolSkills** and executes one when the Orchestrator calls it, returning a
  result. It does **no step sequencing** and has **no recipes**. New Rust = a
  new Tool or ToolSkill, nothing else. ("Rust is created on call" = Rust only
  *executes* on an Orchestrator call; there are normally no recipes for the
  Executioner.)

**Tool invocation — first-class callables (no `__execute_action__`):** tools are
**first-class callables in the Monty namespace**. A recipe's PythonCode calls a
tool directly, e.g. `result = host.resolve_intent(user_input=text)`. Invoking
the binding crosses into Rust, which runs the tool and returns. There is **no
`__execute_action__` string-intrinsic** and **no `__execute_code_step__`** (the
latter was a Model-A per-step relic, retired). `__execute_actions_parallel__` is
**retired** too — Monty is single-threaded, so a parallel helper would degrade to
sequential anyway; "call N tools" is a sequential recipe with N steps.

**The Monty namespace IS the tool registry:** bind = load, call = execute,
unbind = unload at the end of the main-process task. The future MCP bridge hits
the same registry — no Python intrinsic needed. The `__host_call__` 23-arm
`match` (`orchestrator.rs:641-801`) is retired into this registry; host
capabilities register like any first-party tool. Bare Rust helpers
(`intent_system::resolve_intent`, `format_orchestrator_content` /
`parse_orchestrator_channel_steps`) are dissected into registered Tools
(`host.resolve_intent`, `host.compose_orchestrator`), not hidden intrinsics.

The Rust agent-loop stage pipeline (`canonical.rs` stages) is retired as the
production driver entirely; its stage *logic* (prompt assembly, capability
dispatch) is reused as host fns Monty calls. Detail + steps in
`./docs/agents-v3/subplan_problem_stepC_model_a_retirement_of_saved_plan_to_v3.md`.

#### One single main process (ground truth)

From the user input that triggers the InputStage, the **entire processing of
that input is one sole process** — the **main process**, orchestrated and
supervised by Monty from the very start. It is one long-persisting process that
runs **until the user's prompt has been answered**, preferably in the best
possible way (that is what the kohai/sempai system is mainly for). Then history
is stored and the main process exits.

Only the **basic mode's beginning** is built-in (Phase 1: receive the user's
prompt, start the main process, hand off to Phase 2). Everything else is
**Instructions** — a component, most often a **Recipe**, but also possibly an
**Action** or other instruction component. From Phase 2 onward (intent
matching, Matching-Mode, Non-Matching-Mode, validation, component-creation,
kohai-sempai) it is all instruction/recipe-driven, so functionality changes
need **no code changes — only the recipe is altered**.

#### Phase 1 — start (built-in, the one exception)

Monty starts (the information for Monty to run Phase 1 is built-in, not a
recipe — this is the one built-in exception). The main process receives the
user's input and **starts the intent-matching-system**.

#### Phase 2 — intent match (recipe-driven in principle; Rust today)

In principle Phase 2 is run by **Recipes/Instructions** (ideally a second
Python VM), so intent-matching logic can evolve without code changes. For now
the intent system uses the **already-existing Rust** implementation
(`resolve_intent` / `fetch_for_turn` in `brassclaw_engine`); the
recipe/instruction-driven second-VM version is **future work** — only do what
is necessary for a working intent system. The intent system tries to find a
match and returns either a **matching id** (or whatever identifies the match
exactly) or a **"no match"** message back to the orchestrating main process.

#### Phase 3 — dispatch

**Case 1 — Match → Matching-Mode.** The main process receives a component-id
and switches into Matching-Mode:

1. The id is sent to the **composition system**.
2. The composition system fetches and reads the instructions (mostly a
   **Recipe**) belonging to the id. It **splits the rust part and the
   orchestrator part**. It loads whatever is necessary into Rust, assembles
   exactly the python-code + instructions etc. from the **orchestrator part**
   of the recipe, and returns that to the orchestrator.
3. The orchestrator now runs whatever the plan contains **step by step**,
   generates the answer for the user, and posts it back into the chat.
4. History for the process is stored and the main process exits.

Matching-Mode covers both deterministic and LLM-guided recipes — the recipe
itself decides whether the LLM is needed:

- **Tier 0** — deterministic, no LLM. Tool calls are baked into `PythonCode`
  leaves; Monty runs them in the sandbox.
- **Tier 1** — LLM-guided. The recipe hands the LLM prior-knowledge / a plan;
  after the LLM responds, post-LLM tool steps are run by Monty.

**Case 2 — No match → Non-Matching-Mode.** The orchestrator has no direct
instruction, so the user's input is sent to the LLM as a **standard prompt**
assembled by the orchestrator:

1. The **chat history belonging to this exact user-input** (few tokens).
2. The **user's question** (few tokens).
3. A huge prefix called the **base-prompt**, where all the information about
   BrassClaw — about all tools, recipes, skills, etc. — is **precompiled**, so
   the LLM's answer is very fast while having access to information starting at
   roughly **250k tokens** and pushable up to **1 million** prefix tokens.

The main process posts the LLM's answer into the user-chat, then saves a
**thorough history** so the **kohai/sempai system can build new intents,
skills, recipes, tools and other components**, so that **next time the LLM is
not needed anymore**. (Future, planned: the main process is available for LLM
calls **via MCP** to gather information or do whatever the LLM needs — still
routed through the orchestrator, never a classical direct-MCP execution path.)

This is **Tier 2**. It is **not "raw LLM"** — it is a recipe/instruction-driven
non-match routine (only the basic mode's *beginning* is built-in). Because it
is recipe-driven, it can be enhanced with **no code changes**: different prompt
additions for different query types, different prefixes, etc. — only the
recipe is altered.

#### Every LLM prompt is assembled by the orchestrator (ground truth 2)

**Every** LLM prompt — whether it belongs to a Recipe, the non-match path, the
Validation-System, the Component-Creation-System, or the kohai-sempai-system —
is **assembled by the orchestrator**, which tells each system what to do and
how to do it. Every LLM prompt is orchestrated **step by step**: *fetch this
information, now format it for this LLM's needs, now add these sentences to
it*, etc., until the prompt is finally created.

The **kohai is always the last one** working on an LLM prompt, because it
**exchanges the placeholders with the prefix prompts**.

#### The two Tool Systems

A recipe declares the tools it needs; the composition system **binds** them into
the Monty namespace for that run; the Orchestrator calls them by name; they are
**unbound (unloaded) at the end of the main-process task**.

- **Built-in Tools + ToolSkills** — precompiled into the Rust binary from the
  start; bound to static Rust fns.
- **Kohai/sempai-minted Tools + ToolSkills** — compiled as **separate cdylib
  crates**, **loaded dynamically at runtime on demand by a recipe** (via
  `dlopen`), bound into the same namespace, and unloaded at task end.

Same binding mechanism; only the load source differs (static fn vs cdylib).

#### Runtime security — mode-driven, no babysitting of validated components

There is **no universal per-call security wrapper** (the old
`handle_execute_action` policy/lease/gate/event wrapper is retired as a
per-call babysitter). Security is **mode-driven + operator-toggleable**:

- **Matching-Mode (intent match → a Q2+ validated component): ALL runtime
  security OFF.** A validated component follows a distinct, audited path; its
  tool calls — including outbound HTTP — **execute as intended**, with no
  wrapper and no per-tool self-scoping. Validated components are trusted.
- **Non-Matching-Mode (an LLM is involved): wrapper ON.** The policy/lease/
  gate/event layers engage because the LLM generates the path. (Also applies to
  the Validation-System, Component-Creation-System, and kohai/sempai paths —
  anywhere an LLM drives execution.)
- **Q1 components are never accessible.** They sit in the Queue-System; the
  SEC-01 validation gate returns only **Validated (Q2+)** components to Rust /
  the Orchestrator. So Q1 is irrelevant to runtime security — it can never run.
- **WebUI: a global security-settings panel** where an operator can **disable
  each wrapper layer separately** per deployment.

Policy for the LLM-involved path is applied at **bind time** (the composition
system binds only the tools the runtime profile + user grants permit) rather
than per call. Matching-Mode validated components bypass bind-time filtering.

#### Components are the crucial thing

With this architecture, most tasks are performed by the orchestrator on its
own. The most crucial thing is the **components**: if they are made well, a lot
of different tasks can be performed by **different recipes calling the same
components**. The long-term lever is a large library of tiny, reusable
components — more modules and recipes, fewer Rust branches.

#### Recipe syntax — human-readable AND machine-readable

Recipes (+ the composition system) need a **clever dual-nature syntax**: a
**human-readable, logically-constructed** recipe on one hand, and a
**machine-readable exact logic** on the other that **always reproduces the same
results** from the orchestrator and the Rust code. The goal: with everything
running as intended, **no code changes are necessary** to change behaviour —
only the recipe is altered.

**Authoring rules** (enforced at Q1 validation):
- **One leaf skill per approach**: Three skills covering three patterns is better
  than one monolithic skill covering all three. If a tool has N common usage
  patterns, author N leaf skills.
- **One recipe per variant**: Each distinct invocation pattern gets its own Tier-0
  recipe. The intent system routes to the right recipe; the recipe executes
  deterministically without LLM involvement.
- **PythonCode bodies**: Class-22 executors call `__execute_action__()` exactly
  once with a hardcoded tool name. Pure-logic helpers (data transformation) do
  not call `__execute_action__()` at all.
- **Never inline tool calls in LLM prose**: Skills are orchestrator-facing
  narrative. Tool execution happens in PythonCode steps only.
- **Dual-nature fields (Step B):** every recipe carries BOTH natures on the
  same struct — no separate rendering or transpilation:
  - **Machine-readable exact logic (untouched):** `RecipeVariant.step_link` +
    `Recipe.step_descriptions` → IBS `build_instruction` → `BuildInstruction`
    (`rust_steps` + `orchestrator_steps`). Deterministic; never changed by Step B.
  - **Human-readable explanation (concise — "what happens"):**
    `Recipe.description` (recipe-level), `RecipeVariant.description`
    (variant-level — added in Step B), `StepDescriptionEntry.label` +
    `StepEntry.goal` (step-level).
  - **Q1 gate:** a v3-migrated variant (`step_link` present) MUST have a
    non-empty `RecipeVariant.description` (≤ 512 chars); legacy variants
    (`step_link == None`) are exempt. Enforced in
    `RecipeValidator::validate_recipe` (`check_variant_descriptions`).
  - **Read surface:** `RecipeDetail.recipe` is opaque full-engine JSON, so new
    variant fields ride along to the WebUI with no DTO recompile. There is no
    WebUI recipe-authoring route yet (future work).

Full specification: `builtin_stuff_v3.md` (built-in capabilities),
`tomedo_v3.md` (reference implementation for an extension).

### Consumer-Tag Gating (§3.9)

Components carry `consumer_tags[]` that control which agent roles may access them. The `sender_class_code` on a turn maps to a consumer tag; `PostgresSource` enforces a `SEC-01` validation gate — only `Validated` components are returned. Actions (class 11) are exempt from the prior-knowledge token budget.

### 4-Queue Validation Lifecycle (§3.5.1)

| Queue | Code | Meaning |
|-------|------|---------|
| Q1 | `auto` | Auto-extracted; awaiting LLM audit |
| Q2 | `manual` | Operator review required (`05:validator` tag present) |
| Q3 | `revision` | Automated revision by class-09 extension |
| Q4 | `rejection` | Rejected; retained for `q4_retention_days` then wiped |

State transitions enforced by `is_valid_transition` in `brassclaw_product_workflow::recipes`. For Orchestrator (10) and Scaffold (50) classes, `Q1→Q2` requires a clean LLM audit pass.

### Intent System (§3.12)

`resolve_intent` in `crates/brassclaw_engine/src/memory/intent_system.rs` provides 4-class query classification using a single `CASE WHEN` Postgres query against `reborn_intent_inputs`:

- **Class 1** (exact match): returns the matched component ID directly
- **Class 2** (high-confidence): returns the top match
- **Class 3** (disambiguation): returns `Disambiguation` with up to 5 candidates; the orchestrator sends a `role: "disambiguation"` message to the chat UI; the user's selection sends `{disambiguation_choice: component_id}`; `record_disambiguation_choice` stores the selection and increments the score
- **Class 4** (no match / "try it with AI"): falls back to keyword UNION ALL path

### Intent-Driven Retrieval (`fetch_for_turn`)

`PostgresSource::fetch_for_turn` in `retrieval_source.rs` replaces the old "load all docs" path:
1. Calls `resolve_intent` with the user query
2. On `Match`: fetches the exact component by ID from the appropriate class table
3. On `Disambiguation`: returns `FetchForTurnResult::Disambiguation(candidates)` to the orchestrator
4. On `NoMatch` / error: falls back to UNION ALL keyword retrieval (DB-less helpers in `retrieval_dbless.rs`)

### Monty VM Settings (§3.10)

`PgMontyVmSettingsStore` reads/writes `reborn_monty_vm_settings` (V034 migration). `max_duration_secs` bounds the Orchestrator's main-process turn. The legacy `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS` env var is a DB-less fallback only. (The old `ThreadManager` → `ExecutionLoop` → `execute_orchestrator` threading path was Model-A and is retired.)

### PKC Formatting Split (§3.13/§3.14)

`format_prior_knowledge_for_llm()` in `orchestrator.rs` produces deterministic JSON from `PriorKnowledgeResult` items: ordered by `(class_code asc, prompt_uid asc)`, `class_code_label()` for string names, NULL fields omitted. The `formatted_content` surface is the only surface sent to the LLM; raw `content` is never sent.

### Interceptor Architecture (§3.15)

The Sempai/Kohai review loop intercepts each agent turn:
- `RebornLoopDriverHost` saves a `ForensicPacket` on `on_prompt_assembled` (status `AwaitingKohai`)
- `on_kohai_response` closes it (status `Complete`)
- The interceptor tab (WebUI v2 Settings) exposes: Sempai status/mode, "Reassemble base prompt" button, "Pre-warm Sempai KV-cache" button, persona editor, `components_since_rebuild` badge
- Hidden in DB-less mode

### AI Before User Preference (§7 Q18)

`PUT /api/chat/preferences/ai_before_user` persists to `reborn_user_preferences` (V035 migration) via `PgUserPreferenceStore`. The WebUI chat input shows a pill-style toggle (hidden when the preference store is unavailable / DB-less). When enabled, the assistant sends a preliminary response before disambiguation or gate prompts.

### Key Traits

| Trait | Location | Purpose |
|-------|----------|---------|
| `LlmProvider` | `crates/brassclaw_llm/` | Multi-provider LLM integration |
| `EmbeddingProvider` | `crates/brassclaw_embeddings/` | Vector embedding interface |
| `Hook` | `crates/brassclaw_hooks/` | Lifecycle hook points |
| `TurnCoordinator` | `crates/brassclaw_turns/` | Turn coordination contract |
| `HostRuntime` | `crates/brassclaw_host_runtime/` | Host service access |

All I/O is async with tokio. Use `Arc<T>` for shared state, `RwLock` for concurrent access.

**LLM data is never deleted.** All LLM output — context fed to the model, reasoning, tool calls, messages, events, steps — is the most valuable data in the system. Never strip, truncate, or delete it from the database. Mark with timestamps, make filterable, but always retain. In-memory HashMaps are caches; the database (via Workspace) is the source of truth.

### Extension and Auth Invariants

Extension and channel onboarding has two distinct identities that must not be conflated:

- `credential_name`: backend secret identity used for storage, injection, and gate resume
- `extension_name`: user-facing installed extension/channel identity used for setup routing and UI

Rules:

- Never route web setup/configure UI directly from `credential_name`.
- Chat and Settings must use the same setup/configure path for installable extensions/channels.
- Generic auth-card UI is only for non-extension credential prompts or pure OAuth launch prompts.
- If an auth flow is for an installed extension/channel, resolve the `extension_name` once in shared backend logic and carry it through the wire contract.
- New auth/onboarding code must reuse the shared resolver/controller path.

## Project Structure

```
crates/
├── Reborn runtime
│   ├── brassclaw_reborn/           # Runtime, driver registry, boot orchestration
│   ├── brassclaw_reborn_cli/       # brassclaw binary (commands, dispatch)
│   ├── brassclaw_reborn_composition/  # Wiring: capabilities, loops, host access
│   ├── brassclaw_reborn_config/    # Config resolution, profiles, home resolution
│   └── brassclaw_reborn_webui_ingress/  # WebUI v2 gateway adapter and ingress
│
├── Persistence
│   ├── brassclaw_pg/               # Postgres pool, migration runner, SQL migrations V000–V026
│   └── brassclaw_embedded_postgres/ # Self-managed embedded Postgres lifecycle
│
├── Agent loops and engine
│   ├── brassclaw_agent_loop/       # Planned AgentLoop driver
│   ├── brassclaw_engine/           # Engine v2: planning, CodeAct, tool loop
│   │   └── prompts/                # Prompt templates loaded via include_str!()
│   └── brassclaw_engine_types/     # Shared engine types and traits
│
├── LLM and embeddings
│   ├── brassclaw_llm/              # Multi-provider LLM integration
│   │   └── providers/              # openai, anthropic, ollama, nearai, bedrock, tinfoil
│   └── brassclaw_embeddings/       # Embedding providers, hybrid search (FTS + vector + RRF)
│
├── Skills
│   └── brassclaw_skills/           # SKILL.md discovery, scoring, selection, attenuation
│
├── Safety and security
│   └── brassclaw_safety/           # Prompt injection, validation, leak detection, policy
│
├── WebUI v2
│   ├── brassclaw_webui_v2/         # React SPA server, routes, bearer-token auth
│   └── brassclaw_webui_v2_static/  # Static assets for WebUI v2
│
├── Extensions
│   └── brassclaw_extensions/       # Extension lifecycle: install, configure, activate, remove
│
├── Host runtime
│   └── brassclaw_host_runtime/     # Trusted laptop shell access, mount aliases
│
├── Sandbox
│   └── brassclaw_process_sandbox/  # Process sandbox: docker-image validator, capability-lease subprocess gating, scoped filesystems, endpoint allowlists
│
├── MCP
│   └── brassclaw_mcp/              # Model Context Protocol client and session management
│
├── Architecture tests
│   └── brassclaw_architecture/     # Architectural invariant tests
│
└── (additional shared utility crates)

skills/                             # SKILL.md files (trusted user skills)

tests/
├── *.rs                            # Integration tests
├── test-pages/                     # HTML->Markdown conversion fixtures
└── e2e/                            # Python/Playwright E2E scenarios
```

## Module Specs

When modifying a module with a spec, read the spec first. Code follows spec; spec is the tiebreaker.

| Module | Spec |
|--------|------|
| `crates/brassclaw_reborn_cli/` | `crates/brassclaw_reborn_cli/AGENTS.md` |
| `crates/brassclaw_reborn/` | `crates/brassclaw_reborn/CLAUDE.md` |
| `crates/brassclaw_reborn_composition/` | `crates/brassclaw_reborn_composition/CLAUDE.md` |
| `crates/brassclaw_reborn_config/` | `crates/brassclaw_reborn_config/CLAUDE.md` |
| `crates/brassclaw_agent_loop/` | `crates/brassclaw_agent_loop/CLAUDE.md` |
| `crates/brassclaw_llm/` | `crates/brassclaw_llm/CLAUDE.md` |
| `crates/brassclaw_safety/` | `crates/brassclaw_safety/CLAUDE.md` |
| `crates/brassclaw_embeddings/` | `crates/brassclaw_embeddings/AGENTS.md` |
| `crates/brassclaw_reborn_webui_ingress/` | `crates/brassclaw_reborn_webui_ingress/CLAUDE.md` |
| `crates/brassclaw_engine/` | `crates/brassclaw_engine/CLAUDE.md` |
| `tests/e2e/` | `tests/e2e/CLAUDE.md` |

## Token Budget

BrassClaw Reborn targets 7B-14B LLMs within an 8,192-token context window.

| Budget item | Tokens |
|-------------|--------|
| Total context | 8,192 |
| Skills budget | 2,048 |
| Remaining for history, tools, response | ~6,144 |

Compaction is triggered when the in-context history would exceed the budget. Workspace memory (persistent, chunked, searchable) is the mechanism for retaining information across compaction boundaries. Skills are selected to fit within the 2,048-token budget; overflow skills are dropped by priority order.

## Skills System

SKILL.md files extend the agent's prompt with domain-specific instructions.

- **Trust model**: Trusted (user-placed in `~/.brassclaw/skills/` or workspace `skills/`, full tool access) vs Installed (registry, read-only tools)
- **Selection pipeline**: gating (check bin/env/config requirements) -> scoring (keywords/patterns/tags) -> budget (fit within 2,048 tokens) -> attenuation (trust-based tool ceiling)
- **Skill tools**: `skill_list`, `skill_search`, `skill_install`, `skill_remove`

See `.claude/rules/skills.md` for full details.

## Configuration

See `.env.example` for all environment variables.

### Key Reborn Variables

**Bootstrap tier** (fixed set, read before the DB starts — safe as inline `Environment=` in the systemd unit):

| Variable | Default | Purpose |
|----------|---------|---------|
| `BRASSCLAW_REBORN_HOME` | `~/.brassclaw/reborn` | Reborn state root |
| `BRASSCLAW_RUNTIME_PROFILE` | `local_dev` | Per-invocation capability policy: `local_dev` (default), `local_safe`, `local_yolo`, `hosted_safe`, etc. — see `brassclaw runtime-profile list`. Controls the security resolver only; **Postgres is always the storage backend**. `BRASSCLAW_REBORN_PROFILE` (old composition-profile name) is a hard startup error — remove it from any systemd units or env files. |
| `BRASSCLAW_REBORN_LOG` | — | Log filter (e.g., `brassclaw=debug`) |
| `BRASSCLAW_PG_URL` | — | External Postgres URL; optional for single-host local deployments (embedded Postgres used when absent), required for hosted/production |
| `BRASSCLAW_EMBEDDED_PG_PORT` | 5434 | Override embedded Postgres port |
| `BRASSCLAW_EMBEDDED_PG_LISTEN_ADDRESSES` | `127.0.0.1` | Override embedded Postgres listen addresses. Set to `0.0.0.0` to allow LAN connections. **First-boot only**: `postgresql.conf` is written once by `initdb`; to change on an existing cluster, edit `$REBORN_HOME/postgres/data/postgresql.conf` and restart the service. |
| `BRASSCLAW_SECRETS_PASSPHRASE_FILE` | — | Path to master-key passphrase file; set only for passphrase-wrapped ceremony |

**Operator-trusted tier** (data-driven, read by configured name after DB is up — set via `EnvironmentFile=` in the systemd unit):

The *names* of these vars live in `brassclaw_config`; the *values* are read from the environment at runtime and never persisted to the DB. Includes `BRASSCLAW_REBORN_WEBUI_TOKEN`, `BRASSCLAW_REBORN_WEBUI_USER_ID`, provider API keys, OAuth secrets, and trigger auth tokens.

LLM provider configuration is managed via `brassclaw config set` or the first-run wizard and stored in the DB. See `crates/brassclaw_llm/CLAUDE.md`.

## Database

**Postgres is mandatory. There is no non-Postgres production build path.**

All persistence uses Postgres (`brassclaw_pg` crate + embedded Postgres via `brassclaw_embedded_postgres`). In-memory backends are acceptable for **unit tests only** — never in production code or integration paths.

The `postgres` cargo feature in `brassclaw_reborn_composition` is set as a **required default** (`default = ["postgres"]`). Do not add `#[cfg(not(feature = "postgres"))]` fallback paths to production composition or factory code. If you need a non-postgres code path, it belongs only in test fixtures.

The `RebornCompositionProfile` enum and all composition-level profile selection have been removed. There is no `local_dev` vs `hosted` composition split — Postgres is always the backend. `BRASSCLAW_RUNTIME_PROFILE` controls only the per-invocation capability policy (security resolver), never the storage backend. `BRASSCLAW_REBORN_PROFILE` is a hard startup error.

- Treat bootstrap config, DB-backed settings, and encrypted secrets as distinct layers.
- Do not break config precedence, bootstrap env loading, DB-backed config reload, or post-secrets LLM re-resolution.
- All config lives in the `brassclaw_config` Postgres table; provider definitions in `brassclaw_llm_providers`.

## WebUI v2

WebUI v2 is a React SPA served at `/v2` from the `brassclaw` binary.

- Built with the `webui-v2-beta` cargo feature flag
- Static assets embedded via `crates/brassclaw_webui_v2_static/`
- Server routes and bearer-token auth live in `crates/brassclaw_webui_v2/`
- Gateway adapter in `crates/brassclaw_reborn_webui_ingress/`
- Start with `brassclaw serve` (default: `127.0.0.1:3000`)
- For non-loopback listeners, use `serve --host 0.0.0.0` only with a non-yolo profile; `local-dev-yolo` with `--confirm-host-access` refuses non-loopback binds

## Job State Machine

```
Pending -> InProgress -> Completed -> Submitted -> Accepted
    \                \-> Failed
     \-> Failed       \-> Stuck -> InProgress (recovery)
                              \-> Failed
```

## Debugging

```bash
BRASSCLAW_REBORN_LOG=brassclaw=trace cargo run
BRASSCLAW_REBORN_LOG=brassclaw::agent=debug cargo run
RUST_LOG=brassclaw=debug,tower_http=debug cargo run   # v1 with HTTP request logging
```

## Current Limitations

1. Reborn runtime: long-lived daemon/service installation not yet supported
2. Reborn runtime: v1 config, DB, settings, and secrets migration not yet implemented
3. MCP: no streaming support; stdio/HTTP/Unix transports all use request-response
4. ~~WIT bindgen: auto-extract tool schema from WASM is stubbed~~ — removed in Phase 4; tool schemas come from native Extension Manifest v2 / MCP server introspection
5. Built tools get empty capabilities; no UX for granting access
6. No tool versioning or rollback
7. Observability: only `log` and `noop` backends (no OpenTelemetry)
8. `brassclaw` not yet included in cargo-dist release artifacts (see issue #3483)
