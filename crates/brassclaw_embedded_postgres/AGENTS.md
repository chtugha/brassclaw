# brassclaw_embedded_postgres

Embedded PostgreSQL lifecycle management for brassclaw.

## Responsibilities

- Downloads and caches the PostgreSQL 16 binary (via `postgresql_embedded`)
- Verifies SHA-256 checksums of downloaded archives against compiled-in values
- Runs `initdb` on first start to create the data directory
- Detects orphaned servers via `postmaster.pid` PID liveness check
- Manages `pg_ctl start / stop / status`
- Bundles and installs the pgvector shared library for `CREATE EXTENSION vector`
- Writes tuned `postgresql.conf` (`jit=off`, log rotation, conservative memory settings)
- Exposes `ManagedPostgres::start()` and `ManagedPostgres::shutdown()`

## Security notes

- `POSTGRESQL_VERSION` and `GITHUB_TOKEN` env vars are suppressed to prevent
  version substitution by environment injection.
- Checksums in `checksums.rs` are compiled-in; every version bump requires a
  deliberate, reviewed commit updating those values.
- `pg_hba.conf` is written to only allow loopback trust auth (`127.0.0.1/32`).
- Default port 5434 avoids collision with a system Postgres on 5432.

## JIT / MemoryDenyWriteExecute invariant

`jit = off` in `postgresql.conf` is required when `MemoryDenyWriteExecute=yes`
is set in the systemd unit (§7). The JIT compiler allocates executable memory at
runtime, which the MDWE flag forbids. **If you remove `jit = off`, also remove
`MemoryDenyWriteExecute=yes` from the systemd unit — and vice versa.**

## Crate boundary

This crate only manages process lifecycle and PostgreSQL configuration.
SQL schema migrations live in `brassclaw_pg`.
