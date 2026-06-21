# BrassClaw Development Guide

**BrassClaw** is a secure, local-first AI assistant built on the IronClaw Reborn architecture. It targets 7B-14B LLMs within 8,192-token context windows and is implemented as a workspace of approximately 70 Rust crates.

## Build and Test

```bash
cargo fmt                                                              # format
cargo clippy --all --benches --tests --examples --all-features         # lint (zero warnings)
cargo test                                                             # unit tests
cargo test --features integration                                      # + PostgreSQL tests

# Build the Reborn binary with WebUI v2
cargo build --release -p brassclaw_reborn_cli --bin brassclaw-reborn 

# Run with logging
BRASSCLAW_REBORN_LOG=brassclaw=debug cargo run -p brassclaw_reborn_cli --bin brassclaw-reborn
```

E2E tests: see `tests/e2e/CLAUDE.md`.

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

## Architecture

BrassClaw Reborn uses a four-layer model:

1. **Products** — UX surfaces and deployment shapes (CLI, web server, daemon). Products wire together loops, capabilities, and host access. They do not implement agent logic.
2. **Loops** — Agent behavior drivers. A loop manages planning, tool dispatch, turn sequencing, approval gates, checkpointing, retries, and completion. All agentic execution passes through the loop runner.
3. **Kernel** — Authority and policy enforcement. Trust decisions, secret resolution, safety policy, sandboxing, capability grants, and session identity live here. Kernel boundaries are enforced; product and loop code cannot override them.
4. **Infrastructure** — Shared services: LLM providers, persistence, embeddings, WASM runtime, skills, extensions, and observability.

The legacy v1 runtime (`src/`) uses a Channel/Agent/AppBuilder model. Do not mix v1 and Reborn patterns. New work belongs in `crates/`.

### Key Traits

| Trait | Location | Purpose |
|-------|----------|---------|
| `Database` | `src/db/` | Dual-backend persistence abstraction |
| `Channel` | `src/channels/channel.rs` | Normalizes external input to `IncomingMessage` |
| `Tool` | `src/tools/tool.rs` | Extensible tool interface |
| `LlmProvider` | `crates/brassclaw_llm/` | Multi-provider LLM integration |
| `SuccessEvaluator` | `src/evaluation/` | Rule-based and LLM-based success evaluation |
| `EmbeddingProvider` | `crates/brassclaw_embeddings/` | Vector embedding interface |
| `NetworkPolicyDecider` | `src/` | Outbound network policy |
| `Hook` | `src/hooks/` | Lifecycle hook points |
| `Observer` | `src/observability/` | Pluggable event recording |
| `Tunnel` | `src/tunnel/` | Public internet exposure |

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
│   ├── brassclaw_reborn_cli/       # brassclaw-reborn binary (commands, dispatch)
│   ├── brassclaw_reborn_composition/  # Wiring: capabilities, loops, host access
│   ├── brassclaw_reborn_config/    # Config resolution, profiles, home resolution
│   └── brassclaw_reborn_webui_ingress/  # WebUI v2 gateway adapter and ingress
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
├── WASM
│   └── brassclaw_wasm/             # Wasmtime sandbox, host functions, fuel metering
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

src/                                # Legacy v1 runtime — do not modify for Reborn work
├── lib.rs, main.rs, app.rs         # v1 entrypoints
├── agent/                          # v1 agent loop — see src/agent/CLAUDE.md
├── channels/                       # v1 channels (cli, http, web, wasm)
│   └── web/                        # v1 web gateway — see src/channels/web/CLAUDE.md
├── db/                             # Dual-backend persistence — see src/db/CLAUDE.md
├── tools/                          # v1 tool system and registry
├── workspace/                      # Persistent memory system
├── secrets/                        # AES-256-GCM secrets, OS keychain master key
├── safety/                         # Re-export shim for crates/brassclaw_safety
├── sandbox/                        # v1 Docker sandbox
├── worker/                         # Container and job workers
├── orchestrator/                   # Internal HTTP API for sandbox containers
├── setup/                          # 7-step onboarding wizard
├── skills/                         # v1 skills shim
├── hooks/                          # Lifecycle hooks (6 points)
├── tunnel/                         # Tunnel abstraction (cloudflare, ngrok, tailscale)
├── registry/                       # Extension registry catalog and installer
├── observability/                  # Pluggable event/metric recording
└── context/, estimation/, evaluation/, profile.rs, settings.rs

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
| `src/agent/` | `src/agent/CLAUDE.md` |
| `src/channels/web/` | `src/channels/web/CLAUDE.md` |
| `src/db/` | `src/db/CLAUDE.md` |
| `src/tools/` | `src/tools/README.md` |
| `src/workspace/` | `src/workspace/README.md` |
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

| Variable | Default | Purpose |
|----------|---------|---------|
| `BRASSCLAW_REBORN_HOME` | `~/.brassclaw/reborn` | Reborn state root |
| `BRASSCLAW_REBORN_PROFILE` | `local-dev` | Boot profile |
| `BRASSCLAW_REBORN_LOG` | — | Log filter (e.g., `brassclaw=debug`) |
| `LLM_BACKEND` | — | Provider: `openai`, `anthropic`, `ollama`, `nearai`, `bedrock`, `openai_compatible`, `tinfoil` |
| `LLM_BASE_URL` | — | LLM endpoint base URL |
| `LLM_MODEL` | — | Model name or ID |
| `LLM_API_KEY` | — | API key |

LLM backends are documented in `crates/brassclaw_llm/CLAUDE.md`.

## Database

Dual-backend: PostgreSQL + libSQL/Turso. All new persistence features must support both backends.

- Add new DB operations to the shared `Database` trait first, then implement both backends.
- Treat bootstrap config, DB-backed settings, and encrypted secrets as distinct layers.
- Do not break config precedence, bootstrap env loading, DB-backed config reload, or post-secrets LLM re-resolution.

See `src/db/CLAUDE.md` and `.claude/rules/database.md`.

## WebUI v2

WebUI v2 is a React SPA served at `/v2` from the `brassclaw-reborn` binary.

- Built with the `webui-v2-beta` cargo feature flag
- Static assets embedded via `crates/brassclaw_webui_v2_static/`
- Server routes and bearer-token auth live in `crates/brassclaw_webui_v2/`
- Gateway adapter in `crates/brassclaw_reborn_webui_ingress/`
- Start with `brassclaw-reborn serve` (default: `127.0.0.1:3000`)
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
BRASSCLAW_REBORN_LOG=brassclaw=trace cargo run -p brassclaw_reborn_cli --bin brassclaw-reborn
BRASSCLAW_REBORN_LOG=brassclaw::agent=debug cargo run -p brassclaw_reborn_cli --bin brassclaw-reborn
RUST_LOG=brassclaw=debug,tower_http=debug cargo run   # v1 with HTTP request logging
```

## Current Limitations

1. Reborn runtime: long-lived daemon/service installation not yet supported
2. Reborn runtime: v1 config, DB, settings, and secrets migration not yet implemented
3. MCP: no streaming support; stdio/HTTP/Unix transports all use request-response
4. WIT bindgen: auto-extract tool schema from WASM is stubbed
5. Built tools get empty capabilities; no UX for granting access
6. No tool versioning or rollback
7. Observability: only `log` and `noop` backends (no OpenTelemetry)
8. `brassclaw-reborn` not yet included in cargo-dist release artifacts (see issue #3483)
