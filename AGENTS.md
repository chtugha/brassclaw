# Agent Rules

## Purpose and Precedence

`AGENTS.md` is the quick-start routing map for AI coding agents entering the codebase. It is not the full architecture spec. Read the relevant subsystem spec before changing a complex area. When a crate spec exists, treat it as authoritative.

Start with these deeper docs as needed:

- `CLAUDE.md`
- `crates/brassclaw_reborn_cli/AGENTS.md`
- `crates/brassclaw_reborn/CLAUDE.md`
- `crates/brassclaw_reborn_composition/CLAUDE.md`
- `crates/brassclaw_agent_loop/CLAUDE.md`
- `crates/brassclaw_llm/CLAUDE.md`
- `crates/brassclaw_reborn_webui_ingress/CLAUDE.md`
- `tests/e2e/CLAUDE.md`

## Architecture Mental Model

BrassClaw Reborn is organized in three conceptual layers:

- **Products** own UX and surface-level composition. They wire together loops, capabilities, and host access for a specific deployment shape (CLI, web, daemon). Products do not implement agent logic directly.
- **Loops** own agent behavior. They manage planning, tool dispatch, turn sequencing, approval gates, checkpointing, retries, and completion. A loop is the unit of agentic execution. Product code must not implement a second loop or bypass the loop runner.
- **Kernel** owns authority. It controls trust decisions, secret resolution, safety policy enforcement, sandboxing, capability grants, and session identity. Kernel boundaries are not negotiable from product or loop code.

New Reborn work belongs in `crates/`.

## Orchestrator-First, LLM-Minimal Design (Mandatory)

**The orchestrator IS the execution engine. Rust makes tools available. The LLM
is consulted ONLY when a task requires creative reasoning, composition, or
irreversible decisions the user must confirm. Everything else is Tier 0.**

This principle governs all Recipe, Skill, PythonCode, and ToolSkill authoring:

### The Two-Channel Execution Model

```
channel: "rust"           → pre-loads the ToolSkill binding (does NOT execute)
channel: "orchestrator"   → PythonCode calls __execute_action__() to actually run the tool
```

A Tier-0 recipe MUST have both channels. A rust-only Tier-0 recipe is a Q1 hard
error (§tier0-orchestrator-channel Rule 2). The orchestrator **never** calls Rust
directly — it calls Rust tools through `__execute_action__()` in a PythonCode step.

### Tier Decision Hierarchy

1. **Tier 0 first**: Can the task be done deterministically with known inputs? → Author a Tier-0 recipe with a PythonCode executor.
2. **Split by variant**: Each distinct invocation pattern gets its own recipe + intent examples.
3. **Tier 1 only when necessary**: LLM involvement ONLY for creative content, user-composed inputs, or confirmation of irreversible actions.
4. **One leaf skill per approach**: If a tool has 3 common usage patterns, author 3 leaf skills — not one monolithic skill.
5. **10+ intent examples per recipe**: More examples = better routing precision.

### PythonCode Executor Pattern (Canonical Tier-0 body)

```python
# Channel: orchestrator | Class: 22 | No I/O, no imports except stdlib, no network.
# IBS bakes in {{vars.slotN}} values before execution.
# __execute_action__ is provided by the runtime sandbox — not imported.
result = __execute_action__("tool_name", {"param": "{{vars.slot0}}"})
```

### What Forces Tier 1

- Content composition (write_file, apply_patch, user-composed shell commands)
- Ambiguous intent requiring the LLM to decide between alternatives
- Irreversible operations benefiting from LLM confirmation
- User-supplied strings that must be validated before tool dispatch

### Q1 Hard Errors (enforced on all authored components)

- **Rule 1**: Tier-0 `orchestrator_steps` may ONLY contain PythonCode (class 22). Skill bodies are LLM prose — unexecutable without an LLM.
- **Rule 2**: If `llm_call_required == false` AND `rust_steps` has tool bindings, then `orchestrator_steps` MUST contain ≥1 PythonCode UUID.
- **§shell-guard**: Any Recipe using `builtin.shell` where the command string is user-supplied is `llm_call_required: true`. Always.
- **§shell-safe-fixed**: A Recipe using `builtin.shell` with a *fully pre-validated, compile-time-constant command string* (no user-supplied parts) MAY be `llm_call_required: false`.
- **§spawn_subagent-guard**: Any Recipe referencing `builtin.spawn_subagent` is `llm_call_required: true`. Always.

### Extension Authoring Reference

Extension component stacks (Tools, ToolSkills, PythonCode, Leaf Skills, Domain Skills, Recipes, ExtensionCatalogues) are fully specified in:
- `builtin_stuff_v3.md` — built-in capabilities
- `tomedo_v3.md` — tomedo EMR integration example (reference implementation)

## Where to Work

| Area | Location |
|------|----------|
| brassclaw CLI binary | `crates/brassclaw_reborn_cli/` |
| Reborn runtime and driver registry | `crates/brassclaw_reborn/` |
| Composition and wiring | `crates/brassclaw_reborn_composition/` |
| Config resolution and profiles | `crates/brassclaw_reborn_config/` |
| Agent loop driver | `crates/brassclaw_agent_loop/` |
| LLM providers and routing | `crates/brassclaw_llm/` |
| Skills system | `crates/brassclaw_skills/` |
| Security, safety, prompt injection | `crates/brassclaw_safety/` |
| WebUI v2 server (React SPA) | `crates/brassclaw_webui_v2/`, `crates/brassclaw_webui_v2_static/` |
| WebUI ingress / gateway adapter | `crates/brassclaw_reborn_webui_ingress/` |
| Extensions lifecycle | `crates/brassclaw_extensions/` |
| Host runtime shell access | `crates/brassclaw_host_runtime/` (in-kernel capability host + runtime dispatcher; sandboxed subprocess execution via `services/process_executor` and `sandbox_process/`; first-party tools under `first_party_tools/`) |
| Embeddings | `crates/brassclaw_embeddings/` |
| Recipe-Skill-Tool library | `crates/brassclaw_engine/src/memory/` (types, matcher, validator, similarity), `crates/brassclaw_reborn_composition/src/recipe_store.rs` + `recipe_library.rs` (REST store + loop adapter), `crates/brassclaw_turns/src/run_profile/recipe_lookup.rs` (trait) |
| Component catalog (class codes 12–20) | `crates/brassclaw_engine/src/memory/retrieval_source.rs` (`PostgresSource`, `fetch_for_turn`, `FetchForTurnResult`), unified tables `reborn_specs/tool_skills/plans/summaries/lessons/issues/notes` (class codes 12–20) |
| Intent system | `crates/brassclaw_engine/src/memory/intent_system.rs` (`resolve_intent`, 4-class classifier, `record_disambiguation_choice`), `reborn_intent_inputs` table (V028 migration) |
| Monty VM settings | `crates/brassclaw_reborn_composition/src/pg_monty_vm_settings.rs` (`PgMontyVmSettingsStore`, reads/writes `reborn_monty_vm_settings` V034 migration) |
| User chat preferences | `crates/brassclaw_reborn_composition/src/pg_user_preference_store.rs` (`PgUserPreferenceStore`, `reborn_user_preferences` V035 migration) |
| Component import (MemoryDoc migration) | `crates/brassclaw_reborn_composition/src/component_import.rs` (`run_component_import` — migrates legacy `brassclaw_memory_docs` rows into class-specific tables at boot) |
| Interceptor configuration | `crates/brassclaw_interceptor/` (Sempai/Kohai review loop, persona, base-prompt assembly); wired in composition via `InterceptorConfigService` |

When a task touches only `crates/` there is no longer a v1 `src/` tree — all v1 code was removed in Phase 6.

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

- All persistence uses Postgres. In-memory backends are acceptable for unit tests only.
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

**Bootstrap tier** (fixed set, read before the DB starts — set in the systemd unit's `Environment=` block):

| Variable | Purpose |
|----------|---------|
| `BRASSCLAW_REBORN_HOME` | Reborn state root (default: `~/.brassclaw/reborn`) |
| `BRASSCLAW_RUNTIME_PROFILE` | Per-invocation capability policy: `local_dev` (default), `local_safe`, `local_yolo`, `hosted_safe`, etc. — see `brassclaw runtime-profile list`. Controls the security resolver only; does **not** affect which storage backend is used (Postgres is always used). Setting `BRASSCLAW_REBORN_PROFILE` (old composition-profile name) is a hard startup error. |
| `BRASSCLAW_REBORN_LOG` | Log filter for Reborn runtime (e.g., `brassclaw=debug`) |
| `BRASSCLAW_PG_URL` | External Postgres URL. Optional for single-host local deployments (embedded Postgres is used when absent). Required for all non-local `BRASSCLAW_RUNTIME_PROFILE` values. |
| `BRASSCLAW_EMBEDDED_PG_PORT` | Override embedded Postgres port (default: 5434) |
| `BRASSCLAW_EMBEDDED_PG_LISTEN_ADDRESSES` | Override embedded Postgres listen addresses (default: `127.0.0.1`). Set to `0.0.0.0` for LAN access. First-boot only (written to `postgresql.conf` by `initdb`). |
| `BRASSCLAW_SECRETS_PASSPHRASE_FILE` | Path to master-key file; set only when using passphrase-wrapped ceremony |

**Operator-trusted tier** (data-driven, read by configured name after the DB is up — set in `secrets.env` via `EnvironmentFile=`):

The *names* of these env vars are stored in `brassclaw_config`; the *values* are read from the environment at runtime and never persisted. Includes: `BRASSCLAW_REBORN_WEBUI_TOKEN`, `BRASSCLAW_REBORN_WEBUI_USER_ID`, provider API keys, OAuth secrets, trigger auth tokens.

## Build and Test

```bash
# Build the Reborn binary with WebUI v2
cargo build --release --bin brassclaw 

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

- Confirm whether behavior changes require updates to specs, API docs, or `CHANGELOG.md`.
- Run the most targeted tests and clippy checks that cover the change.
- Re-check security-sensitive paths when touching auth, secrets, network listeners, sandboxing, or approvals.
- Keep the final diff scoped to the task. Avoid unrelated file churn.
