# BrassClaw Development Guide

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
| 50 | Scaffold | `reborn_component_catalog` (class 50) |

Legacy `brassclaw_memory_docs` rows are migrated into the appropriate class table at boot by `run_component_import` (`crates/brassclaw_reborn_composition/src/component_import.rs`).

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

`PgMontyVmSettingsStore` reads/writes `reborn_monty_vm_settings` (V034 migration). `max_duration_secs` is threaded from DB through `ThreadManager` → `ExecutionLoop` → `execute_orchestrator` as `max_duration_override: Option<Duration>`. The legacy `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS` env var is a DB-less fallback only.

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
│   └── brassclaw_sandbox/          # Docker execution sandbox, network proxy, allowlist
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
| `BRASSCLAW_SECRETS_PASSPHRASE_FILE` | — | Path to master-key passphrase file; set only for passphrase-wrapped ceremony |

**Operator-trusted tier** (data-driven, read by configured name after DB is up — set via `EnvironmentFile=` in the systemd unit):

The *names* of these vars live in `brassclaw_config`; the *values* are read from the environment at runtime and never persisted to the DB. Includes `BRASSCLAW_REBORN_WEBUI_TOKEN`, `BRASSCLAW_REBORN_WEBUI_USER_ID`, provider API keys, OAuth secrets, and trigger auth tokens.

LLM provider configuration is managed via `brassclaw config set` or the first-run wizard and stored in the DB. See `crates/brassclaw_llm/CLAUDE.md`.

## Database

All persistence uses Postgres (`brassclaw_pg` crate + embedded Postgres via `brassclaw_embedded_postgres`). In-memory backends are acceptable for unit tests only.

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
