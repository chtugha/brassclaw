# Reborn CLI Agent Contract

## Binding preloadable Skill interface (v3)

A Skill is one Tool-usage pattern with prose and explicitly associated PythonCode
exposing a preloadable function interface. Declare public names/signatures,
private helpers/constants and dependencies. Resolve one approved catalogue
snapshot; pin exact interface/code/association/artifact revisions and export
resolution. Load definitions in deterministic dependency-first order, rejecting
cycles/conflicts and effectful initializers. Invoke the pinned export on demand
with typed data; loading is not an invocation or a new Recipe effect step.

Preloaded code does not automatically enable a Tool. Its matching ToolSkill
binding and current kernel checks still apply before every actual dispatch.
Reusable code and immutable constants may be shared; mutable arrays, defaults,
closures, inputs and results remain isolated per task/attempt/invocation. Running
and resumed tasks keep their selected exports when new revisions activate.
Each real Skill has a canonical execution Recipe with a matching command.
MCP tools/list derives from available approved mcp-call-skill-recipes, not raw
Skill rows. Keep the server always running; Kohai connects/advertises to the
provider after final prefix addition just before sending a prompt, then
disconnects that request on the complete answer. Refresh discovery at
startup/restart and qualified Skill/Recipe catalogue changes. Existing calls
keep their advertised contract and normal chat task snapshot.
MCP tools/list gives its exact sentence, variable positions/types, escaping and
valid examples; the model sends the completed command for intent matching.
MCP accepts the completed listed command, opens a new ordinary chat, sends it
as a user message, forwards the correlated chat result and closes the chat.
It accepts no Python and has no direct Monty/IBS/Rust Tool execution connection;
the existing chat ingress, matcher and Recipe runner remain unchanged. No component or
per-call Q1/Q2 is created; only eligible usages are exposed. See [the complete interface contract](../../skills.md#preloadable-function-interface-binding-v3-target).
This is a binding target, not proof of implemented loader/store/runner support.
Current-source observations and historical step-body examples below must be
migrated to this interface before being accepted as updated v3 implementations.


## Recipe architecture routing

Follow [the binding Recipe authoring contract](../../recipe.md) and root AGENTS.md for
v3 component work. Recipes instruct the orchestrator using reusable Tools,
ToolSkills, Skills and small PythonCode components. One referenced component
per Recipe step; internal PythonCode composition is allowed. IBS pins approved
immutable versions in BuildInstruction at task start, including nested includes;
Recipes reference stable UUIDs without versions. Preserve typed inputs/results
and task state across steps and waits. Replacement versions do not alter active
tasks; current global Tool policy still applies at every dispatch. Older scoped,
source-substitution or fresh-step descriptions below are implementation/legacy
notes, not permission to extend those paths as the v3 target. These requirements
remain subject to the documented runtime implementation gaps.

This crate owns the standalone `brassclaw` command surface. Keep it small, explicit, and safe for agents to extend.

## Command layout

- Use one command per file under `src/commands/`.
- Register each command in `src/commands/mod.rs` and dispatch through `Command::execute`.
- Keep `src/cli.rs` as the clap root only: parse top-level CLI and hand off to command modules.
- Put shared process/env boot state in `RebornCliContext` from `src/context.rs`.

## Boundaries

- Commands that need Reborn boot config must receive `RebornCliContext` from dispatch instead of reading env directly. Pure commands that do not need boot config (for example, shell completion generation) must not force Reborn home resolution.
- Keep commands side-effect free unless the command name and issue explicitly require mutation.
- Use `BRASSCLAW_REBORN_HOME` / `~/.brassclaw/reborn`; do not write current v1 state.
- no v1 runtime imports: do not depend on root `brassclaw`, `src/agent`, channels, worker, DB, setup, service, sandbox, or `brassclaw_engine`.
- Do not add workspace dependencies beyond `brassclaw_reborn_composition`, `brassclaw_reborn_config`, `brassclaw_reborn_traces`, and `brassclaw_reborn_webui_ingress` (host-owned WebUI serve lifecycle) without an architecture test update and explicit PR rationale. Provider registry/auth/model UX should enter through the Reborn composition provider-admin facade, not a separate CLI-only path.

## Adding a command

1. Add `src/commands/<name>.rs` with a clap `Args` type and an `execute` method.
2. Add a variant to `commands::Command`.
3. If the command needs boot config, resolve `RebornCliContext` in `commands::Command::execute` and pass it into the command handler.
4. If the command is pure, do not resolve `RebornCliContext` just to run it.
5. Add a binary smoke test in `tests/smoke.rs` that invokes `env!("CARGO_BIN_EXE_brassclaw")`.
6. If the command can touch state, assert it uses Reborn home only and does not create/read v1 DB/settings/secrets.
7. Run:
   - `cargo test -p brassclaw_reborn_cli`
   - `cargo test -p brassclaw_architecture reborn`
   - `cargo clippy -p brassclaw_reborn_cli --all-targets -- -D warnings`

## Beta features

The `webui-v2-beta` Cargo feature compiles in the WebChat v2 HTTP gateway
subcommand (`brassclaw serve`). It is **off by default** so a
default `cargo install` / release build does not link the axum router,
auth middleware, or HTTP/SSE/WS stack at all. Producing a binary that
exposes the v2 surface is an explicit opt-in:

```bash
cargo install --path crates/brassclaw_reborn_cli 
# or, from a workspace checkout
cargo build -p brassclaw_reborn_cli  --release
```

When the feature is off, `brassclaw --help` does not list `serve`
and `brassclaw serve …` returns `error: unrecognized subcommand`.
This is verified by `help_mentions_reborn_commands` in `tests/smoke.rs`,
which only asserts on the `serve` line under `#[cfg(feature =
"webui-v2-beta")]`. Beta-only smoke tests (`serve_help_mentions_host_and_port`,
`serve_fails_closed_when_env_bearer_token_var_is_unset`, etc.) are
themselves feature-gated so default `cargo test -p brassclaw_reborn_cli`
runs do not regress on a missing feature flag.

The descriptor-level "all v2 routes are actually mounted" regression
lives at the composition layer in
`crates/brassclaw_reborn_composition/tests/webui_v2_serve.rs`
(`every_webui_v2_descriptor_is_mounted_on_composed_app`), not here —
that test drives the same `webui_v2_app` the CLI's `serve` hands to
`serve_webui_v2`, so a route that's declared in `webui_v2_routes()` but
forgotten by composition fails the build before the CLI binary smoke
tests run.
