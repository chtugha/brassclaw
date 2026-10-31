---
paths:
  - "crates/**/*.rs"
  - "crates/brassclaw_pg/migrations/**"
---
# Database Rules

## Status & Direction

All persistence uses **PostgreSQL** — the only supported backend. There is no libSQL/Turso dual-backend, no `src/db/` legacy layer, and no `RootFilesystem` mount-based abstraction. All new persistence goes into the `crates/brassclaw_pg/migrations/` folder (Flyway-style `.sql` files) and is accessed via `deadpool-postgres` pools.

---

## Adding a New Operation

1. Identify which store/crate owns the concern (e.g. `brassclaw_turns`, `brassclaw_reborn_composition`, `brassclaw_engine`).
2. Add the async method signature to the relevant trait (usually a `*Store` or `*Repository` trait in the crate).
3. Implement in the `Pg*` concrete type in the same crate (naming pattern: `PgTurnStateStore`, `PgApprovalRequestStore`, etc.).
4. Add a migration if needed:
   - New file: `crates/brassclaw_pg/migrations/VNNN__description.sql`
   - **Version numbering**: number after the highest existing migration, including untracked work. Use `rg --files crates/brassclaw_pg/migrations/` and check the central owner's current reservations before choosing `VNNN`; tracked files alone miss in-progress migrations. Never reuse or insert before an existing version.
5. Follow [the development policy](../../docs/development-policy.md) and
   [testing rules](testing.md). Select the owning crate's actual caller/integration
   target and features; preserve central Cargo ownership. SQL/migration/transaction
   claims need real PostgreSQL coverage of applicable success and failure cases,
   plus relevant affected-package linting. Reuse adequate existing evidence;
   an unavailable database or skipped test remains unverified, not a waiver.

## SQL Dialect

PostgreSQL only. Use:
- `UUID` (not TEXT), `TIMESTAMPTZ` (not TEXT), `JSONB` (not TEXT), `BOOLEAN`, `BIGINT`, `VECTOR`
- Parameterized queries via `tokio-postgres` `client.query(&stmt, &[...])` — never string-formatted SQL.
- `pool.get()` → `client` → `client.query_opt(...)` / `client.execute(...)` (no `sqlx` macros).

## Transaction Safety

Multi-step operations (INSERT+INSERT, UPDATE+DELETE, read-modify-write) MUST be wrapped in a transaction. Ask: "If this crashes between step N and N+1, is the database consistent?" If not, wrap in a transaction.

## Migration Naming

```
V050__recipe_tables.sql
V051__validation_queue.sql
```

Lowercase, descriptive noun-phrase after the `__` separator. The `V` prefix and `__` double-underscore are required. Flyway validates this format on startup.

## Never Delete LLM Output Data

All LLM execution data — thread messages, steps, events, tool call parameters and results — must **never** be deleted from the database. This is the most valuable data in the system. No `DELETE` statements, no `DROP`, no truncation of LLM-generated content.

## Fix the Pattern, Not the Instance

When fixing a bug in one store's SQL, grep for the same pattern across all stores in the affected crate. A partial fix is not a fix.
