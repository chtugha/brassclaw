# Agent Rules

## Purpose and Precedence

`AGENTS.md` is the quick-start routing map for AI coding agents entering the codebase. It is not the full architecture spec. Read the relevant subsystem spec before changing a complex area. When a crate spec exists, treat it as authoritative.

Start with these deeper docs as needed:

- `CLAUDE.md`
- `crates/brassclaw_reborn_cli/AGENTS.md`
- `crates/brassclaw_reborn/CLAUDE.md`
- `crates/brassclaw_reborn_composition/CLAUDE.md`
- `crates/brassclaw_reborn_config/CLAUDE.md`
- `crates/brassclaw_agent_loop/CLAUDE.md`
- `crates/brassclaw_llm/CLAUDE.md`
- `crates/brassclaw_safety/CLAUDE.md`
- `crates/brassclaw_reborn_webui_ingress/CLAUDE.md`
- `src/agent/CLAUDE.md`
- `src/channels/web/CLAUDE.md`
- `src/db/CLAUDE.md`
- `src/tools/README.md`
- `src/workspace/README.md`
- `tests/e2e/CLAUDE.md`

## Architecture Mental Model

BrassClaw Reborn is organized in three conceptual layers:

- **Products** own UX and surface-level composition. They wire together loops, capabilities, and host access for a specific deployment shape (CLI, web, daemon). Products do not implement agent logic directly.
- **Loops** own agent behavior. They manage planning, tool dispatch, turn sequencing, approval gates, checkpointing, retries, and completion. A loop is the unit of agentic execution. Product code must not implement a second loop or bypass the loop runner.
- **Kernel** owns authority. It controls trust decisions, secret resolution, safety policy enforcement, sandboxing, capability grants, and session identity. Kernel boundaries are not negotiable from product or loop code.

The legacy v1 runtime in `src/` follows a different model (Channel/Agent/AppBuilder). Do not mix the two models. New Reborn work belongs in `crates/`.

## Where to Work

| Area | Location |
|------|----------|
| brassclaw-reborn CLI binary | `crates/brassclaw_reborn_cli/` |
| Reborn runtime and driver registry | `crates/brassclaw_reborn/` |
| Composition and wiring | `crates/brassclaw_reborn_composition/` |
| Config resolution and profiles | `crates/brassclaw_reborn_config/` |
| Agent loop driver | `crates/brassclaw_agent_loop/` |
| LLM providers and routing | `crates/brassclaw_llm/` |
| Skills system | `crates/brassclaw_skills/`, `skills/` |
| Security, safety, prompt injection | `crates/brassclaw_safety/` |
| WASM sandbox and tool runtime | `crates/brassclaw_wasm/` |
| WebUI v2 server (React SPA) | `crates/brassclaw_webui_v2/`, `crates/brassclaw_webui_v2_static/` |
| WebUI ingress / gateway adapter | `crates/brassclaw_reborn_webui_ingress/` |
| Extensions lifecycle | `crates/brassclaw_extensions/` |
| Host runtime shell access | `crates/brassclaw_host_runtime/` |
| Embeddings | `crates/brassclaw_embeddings/` |
| Dual-backend persistence | `src/db/` |
| Legacy v1 agent runtime | `src/agent/` — do not modify unless the task explicitly targets v1 |
| Legacy v1 web gateway | `src/channels/web/` — do not modify unless the task explicitly targets v1 |

When a task touches only `crates/` and makes no reference to v1 behavior, do not open or edit files under `src/`.

## Subagent and Loop Rules

- Subagent spawn creates and wires child runs only. It must not implement a second agent loop.
- Child planning, execution, capability calls, checkpointing, gates, retries, and completion must go through the existing loop runner/driver/executor path.
- Host-trusted trigger ingress is sealed by trigger-worker-owned request minting plus private conversation-owned trusted inbound construction.
- Product adapters, product workflow, first-party capabilities, and host-runtime handlers must use untrusted inbound requests and must not mint `TrustedInboundTurnRequest` or call trusted trigger submitter factories.

## Repo-Wide Coding Rules

- No `.unwrap()` or `.expect()` in production code. They are acceptable in tests and for truly infallible invariants (e.g., compiled-in literals, regexes) with a safety comment.
- Keep clippy clean with zero warnings: `cargo clippy --all --benches --tests --examples --all-features -- -D warnings`.
- Prefer `crate::` for cross-module imports. `super::` is fine in tests and intra-module refs.
- Use strong types and enums over stringly-typed control flow when the shape is known.
- Use `thiserror` for error types in `error.rs`. Map errors with context: `.map_err(|e| SomeError::Variant { reason: e.to_string() })?`.
- No `pub use` re-exports unless exposing to downstream consumers.
- Comments for non-obvious logic only.
- Multi-line prompt strings go in `crates/brassclaw_engine/prompts/*.md` and are loaded via `include_str!()`. Never inline large prompt templates as Rust string constants.
- `info!` and `warn!` output appears in the REPL and corrupts the terminal UI. Use `debug!` for internal diagnostics. Background tasks must never use `info!`.

## Database Rules

- New persistence behavior must support both PostgreSQL and libSQL.
- Add new DB operations to the shared DB trait first, then implement both backends.
- Treat bootstrap config, DB-backed settings, and encrypted secrets as distinct layers; do not collapse them.
- Do not break config precedence, bootstrap env loading, DB-backed config reload, or post-secrets LLM re-resolution.

## Security Invariants

- Review any change touching listeners, routes, auth, secrets, sandboxing, approvals, or outbound HTTP with a security mindset.
- Do not weaken bearer-token auth, webhook auth, CORS/origin checks, body limits, rate limits, allowlists, or secret-handling guarantees.
- Treat Docker containers and external services as untrusted.
- Session, thread, and turn state matters. Submission parsing happens before normal chat handling.
- Skills are selected deterministically. Tool approval and auth flows are special paths and must not be mixed into normal chat history.
- Persistent memory is the workspace system, not just transcript storage.

## Testing Rules

- Add the narrowest tests that validate the change: unit tests for local logic, integration tests for runtime/DB/routing behavior, E2E or trace coverage for gateway, approvals, extensions, or other user-visible flows.
- Test through the caller, not just the helper. When a predicate/classifier/transform helper gates a side effect (HTTP, DB write, OAuth flow, UI mutation, tool execution) and has any wrapper or computed input between it and that side effect, a unit test on the helper alone is not sufficient regression coverage. Add a test that drives the actual call site at the integration tier or higher.
- Mocks of multi-arg runtime APIs must capture every argument the production caller passes.

## Key Environment Variables

| Variable | Purpose |
|----------|---------|
| `BRASSCLAW_REBORN_HOME` | Reborn state root (default: `~/.brassclaw/reborn`) |
| `BRASSCLAW_REBORN_PROFILE` | Boot profile: `local-dev`, `local-dev-yolo`, `production`, `migration-dry-run` |
| `BRASSCLAW_REBORN_LOG` | Log filter for Reborn runtime (e.g., `brassclaw=debug`) |
| `LLM_BACKEND` | LLM provider: `openai`, `anthropic`, `ollama`, `nearai`, `bedrock`, `openai_compatible`, `tinfoil` |
| `LLM_BASE_URL` | Base URL for the LLM endpoint |
| `LLM_MODEL` | Model name/ID to use |
| `LLM_API_KEY` | API key for the LLM provider |

## Build and Test

```bash
# Build the Reborn binary with WebUI v2
cargo build --release -p brassclaw_reborn_cli --bin brassclaw-reborn --features webui-v2-beta

# Format
cargo fmt

# Lint a specific crate (zero warnings)
cargo clippy -p <crate_name> --all-targets -- -D warnings

# Lint everything
cargo clippy --all --benches --tests --examples --all-features -- -D warnings

# Unit tests for a specific crate
cargo test -p <crate_name>

# All unit tests
cargo test

# Integration tests (requires PostgreSQL)
cargo test --features integration
```

## Before Finishing

- Confirm whether behavior changes require updates to `FEATURE_PARITY.md`, specs, API docs, or `CHANGELOG.md`.
- Run the most targeted tests and clippy checks that cover the change.
- Re-check security-sensitive paths when touching auth, secrets, network listeners, sandboxing, or approvals.
- Keep the final diff scoped to the task. Avoid unrelated file churn.
