# Subplan — Step 13 of Goals_pre_v3_review.md

## Eliminate the postgres-less e2e/test runtime (`--no-default-features --features libsql`)

> **Parent:** `Goals_pre_v3_review.md` Step 13
> **Goal 2:** "No more postgres-less design. Postgres is mandatory."
> **Why this is a subplan:** This is a large, cross-cutting test-infrastructure change that
> cannot be safely executed or validated without running the full e2e/canary suite. It is
> documented here file-by-file so a future task (with e2e execution capability) can run it
> mechanically.

---

## Problem

The e2e harness, canary scripts, and `Dockerfile.test` build the `brassclaw` binary with:

```
cargo build --no-default-features --features libsql --bin brassclaw
```

This turns the `postgres` feature **OFF**. With `postgres` off:

1. The `#[cfg(not(feature = "postgres"))]` runtime blocks compile. These wire **in-memory**
   stores (e.g. `InMemoryBoundedSubagentGoalStore`, `InMemoryOutboundStateStore`,
   `InMemoryConversationServices`, etc.) instead of the postgres-backed ones.
2. `libsql` is now only a backward-compat alias for `migrate-from-libsql` (a one-way migration
   read path, **not** a storage backend). So the resulting binary has **no real storage
   backend** — it runs entirely in-memory.

This is a **postgres-less runtime** used by e2e/integration tests. It contradicts:
- Goal 2 ("Postgres is mandatory").
- `AGENTS.md` Database Rules ("All persistence uses Postgres. In-memory backends are
  acceptable for unit tests only." — e2e are integration tests, not unit tests).

In **production** the picture is already correct: `default = ["postgres", ...]`,
`PgMemoryDocStore` backs the engine `Store`, and the silent in-memory fallbacks were removed
(Steps 3,4,6,7). The postgres-less path only survives in the test build.

---

## Scope of `#[cfg(not(feature = "postgres"))]` blocks

Confirmed locations (composition crate):

- `crates/brassclaw_reborn_composition/src/runtime.rs` — ~27 occurrences:
  - line 65 (import), 665, 1800–1900 (a large cluster of in-memory store wiring),
  - 2343-area / 2355 (subagent goal store), 2374 (outbound store), 2513 (hooks pool),
  - 2539 (recipe lookup = None), 2553 (interceptor store = None), and more.
- `crates/brassclaw_reborn_composition/src/factory.rs` — lines 824, 953.
- Other crates: `brassclaw_reborn_event_store/src/lib.rs`,
  `brassclaw_reborn_event_store/tests/profile_contract.rs`, `brassclaw_reborn_cli/src/commands/secrets.rs`,
  `brassclaw_reborn_cli/src/commands/serve.rs` (2).

Each `#[cfg(not(feature = "postgres"))]` block is paired with a `#[cfg(feature = "postgres")]`
block. Once the `postgres` feature is non-optional in the test build, the `not(postgres)` blocks
become dead code and must be deleted (the `postgres` blocks are then always compiled).

---

## File-by-file rework

### A. Build commands — switch e2e/canary/test to the `postgres` feature

Replace `--no-default-features --features libsql` with `--features postgres` (or just the
default build) and set `DATABASE_BACKEND=postgres` so the harness spins up embedded Postgres
(testcontainers / embedded PG are already dev-dependencies: `testcontainers-modules` postgres,
`brassclaw_embedded_postgres`).

| File | Line(s) | Current | Target |
|------|---------|---------|--------|
| `Dockerfile.test` | 32 | `cargo build --release --no-default-features --features libsql --bin brassclaw` | `cargo build --release --features postgres --bin brassclaw` |
| `Dockerfile.test` | 54 | `DATABASE_BACKEND=libsql` | `DATABASE_BACKEND=postgres` |
| `Dockerfile.test` | 11 | comment "Build (libsql only — no PostgreSQL dependency)" | update comment to reflect postgres build + embedded PG |
| `tests/e2e/conftest.py` | ~323 | `["cargo", "build", "--no-default-features", "--features", "libsql"]` | `["cargo", "build", "--features", "postgres"]` (ensure the binary build + embedded PG bootstrap) |
| `tests/e2e/CLAUDE.md` | 78 | doc: `cargo build --no-default-features --features libsql` | doc: `cargo build --features postgres` |
| `tests/e2e/README.md` | 24 | `cargo build --no-default-features --features libsql` | `cargo build --features postgres` |
| `scripts/live_canary/common.py` | 91 | `run(["cargo", "build", "--no-default-features", "--features", "libsql"], ...)` | `run(["cargo", "build", "--features", "postgres"], ...)` |
| `scripts/live-canary/upgrade-canary.sh` | 51, 58 | `cargo build --no-default-features --features libsql` | `cargo build --features postgres` |
| `scripts/auth_canary/README.md` | 119 | `cargo build --no-default-features --features libsql` | `cargo build --features postgres` |
| `scripts/replay-snap.sh` | 51 | `--no-default-features ...` | `--features postgres` |
| `docs/reborn/harness/e2e.md` | 133 | `cargo build --no-default-features --features libsql` | `cargo build --features postgres` |

Historical plan docs under `docs/plans/2026-02-24-*.md` reference the old command but are
**historical records** — leave them unchanged (they document past state).

### B. Ensure the e2e harness boots embedded Postgres

- Verify `tests/e2e/conftest.py` and the harness spin up an embedded/testcontainer Postgres
  when `DATABASE_BACKEND=postgres` (the embedded PG crate exists; `BRASSCLAW_EMBEDDED_PG_PORT`
  is configurable). If the harness only knows how to start libsql today, add a postgres boot
  path mirroring `deploy/brassclaw.service`'s embedded-PG usage.
- Set `BRASSCLAW_PG_URL` (or rely on embedded PG default port 5434) for the e2e binary.

### C. Delete the now-dead `#[cfg(not(feature = "postgres"))]` blocks

After the test build always enables `postgres`, delete every `#[cfg(not(feature = "postgres"))]`
block (and keep its paired `#[cfg(feature = "postgres")]` body, dropping the cfg attribute since
postgres is now unconditional). Files:

- `crates/brassclaw_reborn_composition/src/runtime.rs` (~27)
- `crates/brassclaw_reborn_composition/src/factory.rs` (2: lines 824, 953)
- `crates/brassclaw_reborn_event_store/src/lib.rs`
- `crates/brassclaw_reborn_event_store/tests/profile_contract.rs`
- `crates/brassclaw_reborn_cli/src/commands/secrets.rs`
- `crates/brassclaw_reborn_cli/src/commands/serve.rs`

### D. Make `postgres` non-optional (optional cleanup, defer if risky)

- Consider removing the `optional = true` on the postgres deps in root `Cargo.toml` and
  making `postgres` a non-optional feature (or always-on). This is the strongest enforcement of
  "Postgres is mandatory." **Caveat:** this may break the `migrate-from-libsql` standalone
  migration tool if it builds without postgres. Verify the migration module's feature
  requirements first. If risky, defer (Step 15 tracks the transitional feature).

### E. Remove the `libsql`/`migrate-from-libsql` feature (after upgrade cycle)

Per root `Cargo.toml` comment, `migrate-from-libsql` (and the `libsql` alias) is "Removed in the
release after the upgrade cycle completes." Once Step A removes all `--features libsql` build
usage and the upgrade cycle is complete, delete the `libsql`/`migrate-from-libsql` features and
the `dep:libsql` dependency. This is Step 15's final action.

---

## Validation

1. `cargo build --features postgres --bin brassclaw` succeeds.
2. `cargo clippy --all --benches --tests --examples --all-features -- -D warnings` is clean.
3. `cargo test -p brassclaw_reborn_composition` passes (the deleted `not(postgres)` blocks no
   longer need their own compilation).
4. The e2e suite (`tests/e2e`) runs green with `DATABASE_BACKEND=postgres` (embedded PG).
5. Canary scripts (`scripts/live_canary`, `scripts/live-canary`) pass with the postgres build.
6. `Dockerfile.test` builds and the resulting image passes the e2e suite.

## Execution status

**DEFERRED.** This subplan requires an environment that can build the postgres binary and run
the e2e/canary suite to validate. It is documented here for a future task to execute
mechanically. The production codebase is already postgres-only (`default = ["postgres"]`,
`PgMemoryDocStore`, silent fallbacks removed); this subplan only removes the residual
test-build postgres-less path.
