# brassclaw_pg

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
Selected retained loader/store/runner support is implemented; this contract
does not certify every workflow or the complete migration. See
[the current implementation status](../../docs/implementation-status.md).
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

Shared PostgreSQL pool management and schema migration runner for brassclaw.

## Responsibilities

- Builds and returns a `deadpool_postgres::Pool` from a connection URL
- Runs all `V000__`…`V026__` SQL migrations via `refinery`
- Handles migration-history reconciliation for existing deployments (pre-seeding
  history rows for tables that pre-date refinery)
- Owns parsed endpoint/TLS policy for pools and dedicated sessions
- Owns the canonical migration files in `migrations/`

## Migration files

All migrations use `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS`
for idempotency. Each migration is self-contained.

| File | Tables |
|------|--------|
| V000 | pgvector extension + `set_updated_at()` trigger function |
| V001 | `brassclaw_config` |
| V002 | `brassclaw_llm_providers` |
| V003 | `brassclaw_secrets_master`, `brassclaw_secrets` |
| V004 | `brassclaw_runs` |
| V005 | `brassclaw_approvals` |
| V006 | `brassclaw_turns` |
| V007 | `brassclaw_capability_leases` |
| V008 | `brassclaw_session_threads` |
| V009 | `brassclaw_processes`, `brassclaw_process_results` |
| V010 | `brassclaw_extension_manifests`, `brassclaw_extensions` |
| V011 | `brassclaw_resource_accounts` |
| V012 | `brassclaw_checkpoints` |
| V013 | `brassclaw_events`, `brassclaw_audit_log` |
| V014 | `brassclaw_token_settings` |
| V015 | `brassclaw_safety_config`, `brassclaw_capability_permissions` |
| V016 | `brassclaw_memory_docs` |
| V017 | `hooks_predicate_invocations`, `hooks_predicate_values` |
| V018 | `brassclaw_root_filesystem`, `_index_specs`, `_events` |
| V019 | `brassclaw_budget_gates` |
| V020 | `brassclaw_identities`, `_users`, `_email_index` |
| V021 | `brassclaw_triggers`, `brassclaw_local_access` (rename + create) |
| V022 | `brassclaw_conversation_state` |
| V023 | `brassclaw_outbound_policies/subscriptions/deliveries/preferences` |
| V024 | `brassclaw_subagent_goals` |
| V025 | `brassclaw_memory_chat_records` |
| V026 | `brassclaw_forensic_packets` |

## SSL and dedicated sessions

`connection.rs` parses URL and keyword DSNs with `tokio_postgres::Config` and
checks hosts, hostaddr overrides and every mixed endpoint. Remote or ambiguous
endpoints require certificate-validated TLS: explicit `sslmode=disable` is
rejected and `prefer` becomes `require`. Unspecified addresses do not prove
loopback. Explicit local `require` uses TLS; ordinary loopback/Unix connections
retain plaintext compatibility. Missing hosts are rejected: this driver has no
implicit libpq socket-directory default. Native certificate roots are preferred,
with bundled Mozilla roots only when the native store is empty.

Use `pool::build_pool` for the instance's shared pool and
`pool::build_pool_with_max_size` for separately owned utility/credential pools.
Account for aggregate reservations and dedicated listeners; a per-pool default
is not an instance-wide capacity limit. Use `connection::connect` for a genuinely
session-local connection such as LISTEN, and own polling/shutdown explicitly.
None of these helpers retries application SQL or an uncertain effect.
