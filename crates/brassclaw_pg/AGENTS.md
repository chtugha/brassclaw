# brassclaw_pg

Shared PostgreSQL pool management and schema migration runner for brassclaw.

## Responsibilities

- Builds and returns a `deadpool_postgres::Pool` from a connection URL
- Runs all `V000__`…`V026__` SQL migrations via `refinery`
- Handles migration-history reconciliation for existing deployments (pre-seeding
  history rows for tables that pre-date refinery)
- Warns at pool construction time when a non-loopback URL lacks `sslmode=`
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

## SSL

When `build_pool` is given a URL whose host is not loopback (`127.0.0.1` / `::1`
/ `localhost`) and the URL does not contain `sslmode=`, a `warn!`-level message
is emitted. The pool still connects — TLS may be enforced server-side via
`pg_hba.conf` — but the warning is non-suppressible.
