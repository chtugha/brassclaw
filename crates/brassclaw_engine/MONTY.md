# Monty Integration

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


## Current source and execution boundary

The application uses upstream `v1.0.0` plus the versioned BrassClaw control
extension, currently `1.0.0-brassclaw.control.7`. Engine and Monty host dependencies
point to the same local libraries under `vendor/monty-control`:

- [Engine manifest](Cargo.toml)
- [Monty host manifest](../brassclaw_monty_host/Cargo.toml)
- [Control source/ABI and recorded evidence](../../vendor/monty-control/BRASSCLAW.md)

The production allocator belongs to the supervised worker process. Ordinary
`skills-db` startup creates one global orchestrator before ingress; task-local
child contexts preserve isolated values and retained selections within that
service. First-feed compiler sharing is available; full dependency-chain sharing
and complete authored catalogue activation still require their acceptance gates.
An upstream feature or passing interpreter test does not establish a qualified
Skill export, current Tool permission or whole-product support.

## Upgrade process

Read the current manifests, upstream hash record, control patch and compatibility
contract before an upgrade. Preserve upstream integrity and round-trip the local
patch; update the actual path dependencies, independent lockfiles and relevant
ABI/transport contracts together. The source pin is not a Git dependency that
`cargo update -p monty` can advance. Run the relevant real upstream, contained
worker and production callers after a coherent change, following
[development validation policy](../../docs/development-policy.md). Never bypass
retained selections or replay unresolved effects to pass an upgrade.

The generic-tracker compatibility proof under `tests/monty_legacy_tracker` is
historical and test-only. Monty 1.0 removes the old allocation-count limit;
executing-task time, preparation observations and shared live memory are separate
contracts. Use effective runtime/WebUI settings rather than the historical fixed
limits below. See [simplified v3](../../simplified_v3.md).

## Historical v0.0.16 inventory

The following language/module/host inventory and changelog record the older
CodeAct integration as inspected on 2026-04-19. They do not describe the current
Monty 1.0 feature set or the qualified v3 preload profile. Inspect the selected
interpreter, structural validator and actual caller before relying on a feature.

### Historical limitations (pin `v0.0.16`)

These are documented in `prompts/codeact_preamble.md` so the LLM avoids them:

### Syntax not supported
| Feature | Workaround |
|---------|-----------|
| `class Foo:` | Use functions and dicts (host-provided dataclasses work) |
| `with` statements | Use try/finally or direct calls |
| `match` statements | Use if/elif chains |
| `del` statement | Reassign to None |
| `yield` / `yield from` statements | Generator expressions (`x for x in ...`) work; use lists for the rest |
| Type aliases (`type X = ...`) | Omit type annotations |
| Template strings (t-strings) | Use f-strings |
| Complex number literals | Use floats |
| Exception groups (`try*/except*`) | Use regular try/except |

### Limited standard library
`import csv`, `import io`, etc. still fail.

`import os` succeeds but all operations (`os.getenv()`, `Path.*`) are **blocked** by the executor — `OSError: OS operations are not permitted in CodeAct scripts`. This is intentional: agents must use injected tools (`shell`, `read_file`, etc.) instead.

Available built-in modules:
- `asyncio` — `asyncio.gather()` for parallel execution
- `datetime` — date and time handling
- `json` — JSON encoding/decoding
- `math` — standard math functions
- `os.path` — path string manipulation only (no I/O)
- `re` — regex (basic)
- `sys` — system info (limited)
- `typing` — type hints (limited, for annotation only)

### Available builtins
`abs`, `all`, `any`, `bin`, `chr`, `divmod`, `enumerate`, `filter`, `getattr`, `hasattr`, `hash`, `hex`, `id`, `isinstance`, `len`, `map`, `min`, `max`, `next`, `oct`, `ord`, `pow`, `print`, `repr`, `reversed`, `round`, `sorted`, `sum`, `type`, `zip`

### Host-provided functions (always available)
These are injected by the BrassClaw executor, not by Monty:
- `FINAL(answer)` / `FINAL_VAR(name)` — terminate with result
- `llm_query(prompt, context)` — recursive LLM sub-call
- `llm_query_batched(prompts)` — parallel sub-calls
- `rlm_query(prompt)` — full sub-agent with tools
- `globals()` / `locals()` — returns dict of known tool names
- All tool functions (web_search, http, time, etc.)

## Upgrade Changelog

| Date | Pin | Notable changes |
|------|-----|-----------------|
| 2026-04-19 | `v0.0.16` | Mixed `asyncio.gather()` future-resolution panic fix, `hasattr` builtin, and input-safety hardening. |
| 2026-04-10 | `v0.0.11` | JSON perf improvements (~2x loads, ~1.6x dumps), filesystem mounting, Rust-side async API, mount edge case fixes. |
| 2026-03-29 | `7a0d4b7` | Multi-module imports, `datetime` module, `json` module, nested subscript assignment, `str.expandtabs()`. |
| 2026-03-20 | `6053820` | Initial integration. max() kwargs support. |
