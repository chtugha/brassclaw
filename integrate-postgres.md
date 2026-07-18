# integrate-postgres.md — Full PostgreSQL Migration Plan

> **Scope:** Embed a self-managed PostgreSQL process (Option A — `postgresql_embedded`),
> abandon all file-based configuration, make environment variables serviceable only
> from the systemd unit file, and migrate every persistent store (file-based and
> libSQL-based) to PostgreSQL with a better schema design.
>
> **Status:** Plan only — no code changed.
>
> **Review revision 1:** All findings from the first cross-agent review addressed (see §0a).
> **Review revision 2:** All findings from the second cross-agent review addressed
> (C1 two-tier env model, C2 production headless passphrase, C3 systemd wizard guard,
> H1 tenant_id synthesis, H2 EnvironmentFile secrets, M1 rewrap strategy names,
> M2 pg_cron §4.13, M3 serve-only retention, M4 event_id-is-TEXT rationale, M5 process_results integrity,
> L1 full default feature line, L2 Phase 2 db_config.rs wording, L3 hardening directives added to §7).
> **Review revision 3:** All findings from the third cross-agent review addressed
> (C1 jit=off + hardened-unit test, C2 sudo -u brassclaw + file ownership, C3 fresh vs upgrade split,
> H1 brassclaw_secrets table purpose clarified, H2 algorithm default raw-key-on-disk,
> M1 wrapping threat-model candor + DR backup gap, M2 passphrase vs passphrase-file clarified,
> M3 LoadCredential note + abstraction path, M4 SystemCallFilter PG compat note,
> L1 §0 #2 bootstrap list updated, L2 operator-trusted env tier + Owner ID/WEBUI_USER_ID decoupled,
> L3 pg_cron removed from §4.18 occurred_at comment).
> **Review revision 4:** All findings from the fourth cross-agent review addressed
> (MH CLI PG lifecycle §6.4 + rewrap schema-first, M /etc/brassclaw ReadWritePaths removed,
> M §8.1 step 6 local-dev UPSERT both wrapped_key+algorithm, L1 §7.0 prerequisites block,
> L2 §6.5 --yes flag mapping, L3 rewrap vs rotate clarified in §4.4).
> **Review revision 5:** All findings from the fifth cross-agent review addressed
> (MH rewrap key-source invariant + fail-closed + passphrase-change unwrap, M §6.4 conditional
> PG shutdown, M passphrase-change old-passphrase required, L1 rotate old-version retirement,
> L2 §6.5 --no-llm + per-provider api-key-env defaults, L3 §7.0 RHEL nologin path).
> **Review revision 6:** All findings from the sixth cross-agent self-review addressed
> (C1 brassclaw_resource_accounts optimistic locking — version column + CAS UPDATE,
> C2 brassclaw_root_filesystem missing tenant_id for multi-tenant isolation,
> H1 brassclaw_approvals run_id FK missing ON DELETE clause,
> H2 BRASSCLAW_PG_URL SSL mode note for external production PG,
> H3 brassclaw_config *_env keys agent-write-gate security note,
> M1 §0 body "Secret tier" → "Operator-trusted env tier" terminology fix,
> M2 brassclaw_config.value non-string type serialization note,
> M3 brassclaw_extensions installed_at vs created_at design-philosophy consistency,
> M4 embedded PG connection URL pg_hba.conf trust-auth note,
> M5 brassclaw_turns missing tenant_id index,
> L1 Phase 7 depends on Phase 6 ordering constraint explicit note).
> **Review revision 7:** All findings from the seventh external review addressed
> (MH1 rewrap tenant-resolution unspecified — 4-step resolution + --tenant flag + §7.1/§7.2 explicit
> --tenant + Phase 7 non-default-tenant integration test + risks table row,
> M1 save_config_key missing ConfigWriteContext parameter — full signature + enum + §5.5 + §12 table update,
> M2 resource_accounts CAS first-write path — INSERT ON CONFLICT upsert pattern added to §4.12,
> L1 rewrap passphrase-change shell invocation — --old-passphrase-file flag + passphrase-read fallback chain,
> L2 ON DELETE RESTRICT comment broadened to approvals/turns/checkpoints + explicit FK clauses on turns + checkpoints,
> L3 raw-key file boot_tenant association documented in §4.4).
> **Review revision 8:** All findings from the eighth external review addressed
> (M1 RebornConfigFile::load() removal contradicts §4.4 rewrap step 2 + §8.1 step 3 — load()
> retained behind migrate-from-libsql feature; removal deferred to next release; §5.4, Phase 2
> checklist, Phase 7 checklist, §12 file table all updated,
> M2 §4.12 first-write DO UPDATE lost-update bug — replaced with INSERT DO NOTHING + read-back
> + CAS UPDATE two-step pattern that preserves CasSnapshotStore retry semantics,
> L1 §7.2 grep instruction wrong for TOML section syntax — corrected to grep tenant).
> **Review revision 9:** All findings from the ninth external review addressed — plan
> is now implementation-ready.
> (L1 §8.1 steps 3-5 not gated behind migrate-from-libsql — added module-level
> #[cfg(feature = "migrate-from-libsql")] note at top of §8.1; steps 1-2 clarified
> as unconditional,
> L2 Phase 2 missing AgentSession-succeeds-for-non-env-keys test — added third
> ConfigWriteContext test asserting gate is suffix-scoped only).

---

## 0a. Exceptions to Current AGENTS.md Rules (sign-off required)

This plan intentionally supersedes two standing rules in `AGENTS.md`/`CLAUDE.md`.
Both must be explicitly rewritten as part of Phase 9 and need sign-off before
implementation begins.

**Rule being retired:**
> "New persistence behavior must support both PostgreSQL and libSQL.
>  Add new DB operations to the shared DB trait first, then implement both backends."

**Rationale for retirement:** The embedded-PG model (§2) makes libSQL redundant as
a runtime substrate. Maintaining dual backends requires every new store to be written
twice, tested in a parity suite, and kept in sync — a constraint that was justified
when libSQL was the simpler local-dev option. With an auto-managed embedded Postgres,
the cost of dual backends is no longer offset by a simpler local path.

**Specific AGENTS.md/CLAUDE.md lines to rewrite in Phase 9:**
- `AGENTS.md` "Database Rules" section: remove the dual-backend mandate; replace with
  "All persistence uses Postgres. In-memory backends are acceptable for unit tests only."
- `CLAUDE.md` "Database" section: same.
- `CLAUDE.md` "Key Traits" table: remove `Database` row (v1 trait no longer exists
  in any non-test path).
- `CLAUDE.md` `src/` structure section: purge all `src/db/`, `src/channels/`,
  `src/agent/`, `src/workspace/`, `src/sandbox/`, `src/registry/`, `src/tunnel/` docs —
  these describe v1 code removed in Phase 6 that is still documented as if live.

---

## 0. Guiding Principles

1. **Single source of truth.** All mutable state lives in Postgres. The filesystem is
   only used for the `BRASSCLAW_REBORN_HOME` pointer and binary artifacts (the embedded
   PG data directory, compiled skills bundles).
2. **Two-tier env var model.** The runtime reads env vars in two distinct tiers:
   - **Bootstrap tier (fixed set, read before DB starts):** `BRASSCLAW_REBORN_HOME`,
     `BRASSCLAW_REBORN_PROFILE`, `BRASSCLAW_PG_URL`, `BRASSCLAW_EMBEDDED_PG_PORT`,
     `BRASSCLAW_REBORN_LOG`, `BRASSCLAW_SECRETS_PASSPHRASE_FILE`. These are the
     only vars that affect startup before Postgres is available (the last is
     production-only; needed to unwrap the master key before the secrets store
     is accessible).
   - **Operator-trusted env tier (data-driven, read by configured name after DB is up):** WebUI
     token, WebUI user-id, provider API keys, OAuth client secrets, trigger auth
     tokens, traces bearer token. The *names* of these env vars are stored in
     `brassclaw_config`; the *values* are read from the environment at runtime and
     never persisted. Because the names are operator-configurable, the set of secret
     env vars is unbounded — the runtime cannot enforce a closed allowlist here.
     The security boundary is: **config controls which names are read; values never
     touch the DB or any log**.
   All other configuration lives in the `brassclaw_config` Postgres table.
3. **Embedded Postgres is the default.** No external Postgres required. On first run,
   `postgresql_embedded` downloads the platform binary, runs `initdb`, and starts the
   server inside `$BRASSCLAW_REBORN_HOME/postgres/`. An external Postgres URL
   (`BRASSCLAW_PG_URL`) overrides this for production deployments.
4. **Dual-backend invariant is dissolved.** The libSQL / file-system dual path is
   eliminated. Every store implements the shared trait against Postgres only.
   In-memory backends are kept for unit tests only (behind `#[cfg(test)]`).
   *This directly supersedes the AGENTS.md dual-backend rule — see §0a.*
5. **No breaking change to agent-facing contracts.** Trait boundaries
   (`TurnCoordinator`, `HostRuntime`, `SecretStore`, etc.) stay identical. Only
   concrete implementations are swapped.
6. **Migration is non-destructive.** The first boot reads any existing
   file-based state and writes it to Postgres before removing the files.
   Down-migrations are not required.

---

## 1. Inventory of Everything Being Removed or Migrated

### 1a. File-based stores (all become Postgres tables)

| Current file / path | Data it holds | New location |
|---|---|---|
| `$REBORN_HOME/config.toml` | Boot profile, LLM slot selections, WebUI settings, budget defaults, trigger poller settings, skill/token flags | `brassclaw_config` table |
| `$REBORN_HOME/providers.json` | Custom LLM provider definitions | `brassclaw_llm_providers` table |
| `$REBORN_HOME/sempai_provider.json` | Sempai role selection (`provider_id`, `model`) | `brassclaw_config` table (`sempai.*` keys) |
| `$REBORN_HOME/.reborn-local-dev-secrets-master-key` | AES-256 master key for the secret store | `brassclaw_secrets_master` table (key encrypted by hardware keyring or derived from PBKDF2 of a short passphrase at first-run) |
| Virtual path `/runs/*` | Run state records | `brassclaw_runs` table |
| Virtual path `/approvals/*` | Approval requests | `brassclaw_approvals` table |
| Virtual path `/turns/*` | Turn state | `brassclaw_turns` table |
| Virtual path `/capabilities/*` | Capability leases | `brassclaw_capability_leases` table |
| Virtual path `/processes/*` | Process records | `brassclaw_processes` table |
| Virtual path `/process-results/*` | Process results | `brassclaw_process_results` table |
| Virtual path `/extensions/*` | Extension installation state | `brassclaw_extensions` table |
| Virtual path `/resources/*` | Resource governor / budget state | `brassclaw_resource_accounts` table |
| Virtual path `/checkpoints/*` | Agent loop checkpoint blobs | `brassclaw_checkpoints` table |
| Virtual path `/sessions/*` | Session thread service state | `brassclaw_session_threads` table |
| Virtual path `/events/*` | Durable event log | `brassclaw_events` table |
| Virtual path `/audits/*` | Durable audit log | `brassclaw_audit_log` table |
| Virtual path `/system/extensions/*` | Extension manifests (TOML) | `brassclaw_extension_manifests` table |
| Root filesystem generic entries (libSQL `root_filesystem_entries`) | All VFS blobs | Merged into domain tables above; `root_filesystem_entries` fallback kept for unrecognised paths |
| libSQL `settings` table (token settings) | Per-provider token budget settings | `brassclaw_token_settings` table |
| libSQL `safety_config` table | Safety rules & capability permissions | `brassclaw_safety_config` + `brassclaw_capability_permissions` tables |
| libSQL `memory_docs` table | Reduction rules, skill MemoryDocs | `brassclaw_memory_docs` table |
| `hooks_predicate_invocations` / `hooks_predicate_values` (both libSQL and Postgres) | Hook predicate state | Same tables, but now the canonical Postgres backend; libSQL path removed |

### 1b. Environment variables removed from runtime config

All of the following stop being read at runtime. Their non-secret metadata moves to the
`brassclaw_config` table (set via the CLI wizard or `config set`):

- `LLM_BACKEND`, `LLM_MODEL`, `LLM_BASE_URL` (→ `brassclaw_config` LLM slot)
- `BRASSCLAW_REBORN_GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_ID` (→ `brassclaw_config` oauth.google)
- `DATABASE_BACKEND`, `LIBSQL_PATH`, `LIBSQL_URL` (eliminated — only Postgres now)

**API keys** (`LLM_API_KEY`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc.) and
the **WebUI bearer token** (`BRASSCLAW_REBORN_WEBUI_TOKEN`) keep their env-var
model — see §1c and the model change note below.

> **Secret-store split:** Operator-sourced secrets (API keys, WebUI token/user-id,
> trigger/traces tokens) remain **env-only**: the env-var name is stored in
> `brassclaw_config`; the value is read from the environment at serve time and
> never written to `brassclaw_secrets`. The encrypted `brassclaw_secrets` table
> holds only **runtime-obtained credentials** — OAuth refresh/access tokens and
> credential-broker secrets acquired during auth flows (e.g. `FilesystemCredentialBroker`
> in `crates/brassclaw_secrets`). This is what justifies the master-key ceremony:
> a DB-level breach cannot expose OAuth tokens without the passphrase file on the
> app host. See §4.4 for the schema and §H1 rationale.

### 1c. Environment variables (two tiers)

#### Bootstrap tier — fixed set, read before DB starts

| Variable | Purpose | Default |
|---|---|---|
| `BRASSCLAW_REBORN_HOME` | Reborn state root | `~/.brassclaw/reborn` |
| `BRASSCLAW_REBORN_PROFILE` | Boot profile | `local-dev` |
| `BRASSCLAW_PG_URL` | External Postgres URL (overrides embedded PG) | unset → use embedded |
| `BRASSCLAW_EMBEDDED_PG_PORT` | Override embedded PG port (default 5434) | unset |
| `BRASSCLAW_REBORN_LOG` | Log filter | unset |
| `BRASSCLAW_SECRETS_PASSPHRASE_FILE` | Path to 0600 file holding Argon2id passphrase for master-key unwrap (production profile only — see §4.4) | unset |

These six vars are the only ones read before the DB is available (or during
unwrap before secrets are accessible). They are set in the systemd unit's
`EnvironmentFile` (or shell profile for local-dev). All others are ignored at
the bootstrap stage.

#### Operator-trusted env tier — data-driven, read by configured name after DB is up

> **Renamed from "secret tier":** This tier carries both secret values (API keys,
> tokens) and non-secret identity values (WebUI user-id) that must not be
> DB-influenceable. The security property is that an attacker who can write
> `brassclaw_config` rows cannot redirect which identity or token the serve
> process trusts — those values come from the operator's own environment.

The *name* of each env var is stored in `brassclaw_config`; the *value*
is read from the process environment at serve time and never persisted. Because
names are operator-configurable, the set is open-ended — there is no closed
allowlist. The security guarantee is: **values never touch the DB, config file,
or any structured log output.**

> **LoadCredential alternative (M3):** systemd 250+ supports
> `LoadCredential=secrets-passphrase:/etc/brassclaw/master.key`, which presents
> the file at `$CREDENTIALS_DIRECTORY/secrets-passphrase` readable by the service
> user (systemd handles the ownership) — resolving the C2 ownership issue without
> `sudo -u brassclaw rewrap`. Phase 3 implementation should read
> `$CREDENTIALS_DIRECTORY/secrets-passphrase` first (if set), falling back to
> `BRASSCLAW_SECRETS_PASSPHRASE_FILE`. The §7 unit can adopt `LoadCredential=`
> once Phase 3 implements the abstraction; the current `EnvironmentFile=` path
> remains valid for older systemd.

| Config key | Default env var name | What it holds |
|---|---|---|
| `webui.token_env` | `BRASSCLAW_REBORN_WEBUI_TOKEN` | WebUI bearer token (required; secret) |
| `webui.user_id_env` | `BRASSCLAW_REBORN_WEBUI_USER_ID` | WebUI owner user-id (required; non-secret identity value, but must not be DB-influenceable — see note) |
| `llm.<slot>.api_key_env` | e.g. `OPENAI_API_KEY` | LLM provider API key |
| `oauth.<provider>.client_secret_env` | operator-named | OAuth client secret |
| `trigger_poller.<name>.auth_token_env` | operator-named | Trigger auth token |
| `tracing.bearer_token_env` | operator-named | Traces/gateway bearer token |

> **WebUI user-id vs Owner ID:** `webui.user_id_env` (env tier) and
> `identity.default_owner` (in `brassclaw_config`) are different values in
> different stores. `identity.default_owner` is the config-layer default owner for
> new sessions; `BRASSCLAW_REBORN_WEBUI_USER_ID` (env) is the identity the serve
> process asserts for bearer-token WebUI auth. They should match in a standard
> single-user deployment, but operators must keep them consistent manually — the
> wizard prompts for both (Step 2 and Step 3) and warns if they diverge.

> **Security: agent must not be able to write `*_env` config keys.** The
> `webui.token_env`, `webui.user_id_env`, `llm.<slot>.api_key_env`,
> `oauth.<provider>.client_secret_env`, `trigger_poller.<name>.auth_token_env`,
> and `tracing.bearer_token_env` config keys are security-critical: they control
> *which* environment variable the serve process trusts for auth, identity, and
> secret resolution. An agent capability that can write arbitrary `brassclaw_config`
> rows could rename these keys to arbitrary variable names — effectively rerouting
> which env value is read for auth. **The agent loop must never be granted a
> capability to write `brassclaw_config` rows whose keys match the `*_env` pattern.**
> Changes to these keys require explicit operator intent (CLI `config set` or the
> first-run wizard). This invariant is enforced at the `db_config::save_config_key`
> layer: the function rejects writes to any key ending in `_env` unless the caller
> holds an operator-tier auth context (not an agent/session context).
> Phase 2 must add a test that asserts agent-sourced `save_config_key` calls are
> rejected for `*_env` keys.

Non-secret WebUI config that was previously env-based is moved to
`brassclaw_config` so no env var is required at serve time:

| Moved to config key | Previously | Notes |
|---|---|---|
| `webui.base_url` | `BRASSCLAW_REBORN_WEBUI_BASE_URL` | SSO callback base URL — not a secret |
| `webui.allowed_email_domains` | `BRASSCLAW_REBORN_WEBUI_ALLOWED_EMAIL_DOMAINS` | SSO admission list — not a secret |

> **Production passphrase:** `BRASSCLAW_SECRETS_PASSPHRASE_FILE` is required
> for unattended production boot. Without it, the only working strategy is
> `keychain`, which requires a desktop session unavailable under headless
> systemd. See §4.4 for the full strategy table and one-time setup procedure.

---

## 2. New Crate: `brassclaw_embedded_postgres`

### 2.1 Location

```
crates/brassclaw_embedded_postgres/
├── AGENTS.md
├── Cargo.toml
└── src/
    ├── lib.rs           # pub struct ManagedPostgres; pub fn start()
    ├── config.rs        # EmbeddedPostgresConfig (port, data_dir, bin_cache_dir)
    ├── download.rs      # uses `postgresql_embedded` crate to fetch+cache PG binary
    ├── initdb.rs        # runs initdb, writes postgresql.conf tuning
    ├── pgctl.rs         # pg_ctl start/stop/status wrappers
    ├── health.rs        # retry TCP connect until ready
    └── error.rs         # EmbeddedPostgresError (thiserror)
```

### 2.2 Key decisions

- **Postgres version pinned to 16.x.** Written as a `const` in `download.rs`;
  updating it is a deliberate, reviewable commit.

- **Binary cached in `$REBORN_HOME/postgres/bin/`.** Not bundled in the Rust
  binary. Downloaded exactly once from the zonky PostgreSQL distribution via
  `postgresql_embedded`. **Checksum verification is implemented by this crate,
  not by `postgresql_embedded` itself** (the upstream crate does not verify
  checksums): after download, `download.rs` computes SHA-256 of the archive and
  compares it against a `const` compiled into the binary. If the digest
  mismatches, the archive is deleted and the process aborts. The pinned digest
  list lives in `crates/brassclaw_embedded_postgres/src/checksums.rs` and must
  be updated for every version bump via a deliberate, reviewed commit. The
  `POSTGRESQL_VERSION` and `GITHUB_TOKEN` env vars that `postgresql_embedded`
  normally reads are suppressed so an attacker who can set env cannot change the
  downloaded version. Trust root: GitHub TLS + zonky publisher + compiled-in
  SHA-256 — protects against CDN compromise but not a compromised zonky build
  pipeline; Sigstore/cosign can be added later if upstream adopts it.
  **Production recommendation:** use `BRASSCLAW_PG_URL` to point at an
  operator-managed Postgres where supply-chain trust is a hard requirement.

- **Data directory `$REBORN_HOME/postgres/data/`.** Created by `initdb` on
  first start. If a `postmaster.pid` already exists in the data dir, startup
  checks whether the recorded PID is still alive (kill -0): if yes, the server
  is already running — reuse it; if no, remove the stale PID file and restart.
  This handles the SIGKILL / crash-orphan case correctly. `initdb` is skipped
  whenever the data dir already exists and is non-empty.

- **Port `5434` by default.** Avoids collision with system Postgres on 5432.
  Configurable via `BRASSCLAW_EMBEDDED_PG_PORT` env var (§1c) or
  `EmbeddedPostgresConfig::port`. On startup, if the port is already in use
  (TCP connect succeeds), the process aborts with:
  `"embedded PG port 5434 in use — set BRASSCLAW_PG_URL or BRASSCLAW_EMBEDDED_PG_PORT"`.

- **`postgresql.conf` tuning for single-user agent workload** (conservative —
  suitable for a laptop or a modest server):
  ```
  max_connections = 20
  shared_buffers = 32MB
  work_mem = 4MB
  max_wal_size = 1GB
  autovacuum = on
  # JIT disabled: pays off for OLAP scans, not the small OLTP-ish queries
  # this server runs. Also required for MemoryDenyWriteExecute=yes in the
  # systemd unit (§7) — PG JIT compiles into executable memory at runtime,
  # which MDWE forbids. Do not enable JIT without removing MDWE.
  jit = off
  log_destination = 'stderr'
  logging_collector = on
  log_directory = 'log'
  log_filename = 'postgresql-%Y-%m-%d.log'
  log_rotation_age = 1d
  log_rotation_size = 50MB
  log_truncate_on_rotation = on
  log_min_duration_statement = 1000
  ```
  `log/` is inside the data directory; the 50 MB rotation cap prevents
  disk fill. Operators can edit `postgresql.conf` directly for tuning.
  **If you remove `MemoryDenyWriteExecute=yes` from the §7 unit, also
  remove `jit = off` here — the two settings must be changed in tandem.**

- **Connection URL format:** `postgresql://brassclaw@127.0.0.1:5434/brassclaw`
  The database and role are created by an init SQL script run after `initdb`.
  **`pg_hba.conf` trust auth:** the init script also writes a `pg_hba.conf` entry:
  ```
  host  brassclaw  brassclaw  127.0.0.1/32  trust
  ```
  This allows passwordless TCP connection from localhost only — safe for an
  embedded loopback server owned by the service user. No password is required in
  the URL. This entry is written unconditionally by `initdb.rs` and must not be
  modified to accept non-loopback connections.

- **`BRASSCLAW_PG_URL` SSL requirement for external Postgres:** when
  `BRASSCLAW_PG_URL` points at an external or remote Postgres (not `127.0.0.1`
  or `::1`), operators **must** append `?sslmode=require` (or `sslmode=verify-full`
  for mTLS) to the URL:
  ```
  BRASSCLAW_PG_URL=postgresql://brassclaw@db.example.com:5432/brassclaw?sslmode=require
  ```
  AGENTS.md: "Review any change touching listeners, auth, secrets, or outbound HTTP
  with a security mindset." Connections to a remote PG without TLS expose
  `brassclaw_secrets` ciphertext, config, and session state in transit.
  The `brassclaw_pg::pool::build_pool` function must log a `warn!`-level message
  if the URL host is not a loopback address and the URL does not contain `sslmode=`:
  ```
  [warn] BRASSCLAW_PG_URL points to non-loopback host without sslmode — TLS is strongly recommended
  ```
  (The pool still connects, to avoid breaking environments with TLS enforced
  server-side via `pg_hba.conf`; but the warning is non-suppressible.)

- **Explicit shutdown before pool drop.** `ManagedPostgres` exposes a
  `shutdown()` async method that the composition root calls *after* closing
  the connection pool. `Drop` retains a best-effort `pg_ctl stop -m fast` as
  a last-resort fallback only — it must never be the primary shutdown path
  because a blocking `pg_ctl` inside `Drop` while open pool connections exist
  can deadlock.

- **`BRASSCLAW_PG_URL` override:** If this env var is set, the embedded
  server is never started. The URL is used directly for the pool.

---

## 3. New Crate: `brassclaw_pg`

All shared Postgres pool management, migration runner, and the canonical
`deadpool_postgres::Pool` constructor live here. This replaces the scattered
`#[cfg(feature = "postgres")]` blocks in individual crates.

```
crates/brassclaw_pg/
├── AGENTS.md
├── Cargo.toml            # deadpool-postgres, tokio-postgres, refinery, thiserror
└── src/
    ├── lib.rs            # pub struct PgPool(deadpool_postgres::Pool); re-exports
    ├── pool.rs           # build_pool(url: &str) -> Result<PgPool, PgError>
    ├── migrations.rs     # run_migrations(&pool) -> Result<(), PgError>
    └── error.rs
```

`migrations.rs` uses `refinery` to run all SQL migration files embedded via
`include_str!`. Migration files live in `crates/brassclaw_pg/migrations/`
numbered `V000__` … `Vnnn__`. Each crate that currently has its own migration
folder (`brassclaw_hooks_postgres/migrations/`) has its SQL moved here.

**Migration-history reconciliation for existing deployments.** The existing hooks
and inline-DDL tables (`hooks_predicate_invocations`, `hooks_predicate_values`,
`root_filesystem_entries`, `memory_docs`, `settings`, `safety_config`,
`capability_permissions`) were applied via idempotent `CREATE TABLE IF NOT EXISTS`
batches, not via refinery. Refinery tracks applied migrations in
`refinery_schema_history` and would try to re-run their consolidated SQL on
deployments where these tables already exist. To prevent that:

1. On first run, `brassclaw_pg::migrations::run_migrations` checks whether
   `refinery_schema_history` is empty **and** whether any of the
   already-existing-in-the-wild tables are present.
2. If so, it inserts pre-seeded history rows marking those migrations as
   already applied (using their compiled-in checksums).
3. Only then does it run the normal `refinery::embed_migrations!` pass.

All refinery migration SQL also uses `CREATE TABLE IF NOT EXISTS` and
`CREATE INDEX IF NOT EXISTS` so they remain safe to re-run in edge cases.

---

## 4. Schema Design

### 4.1 Design philosophy

- **JSONB for flexible document fields, typed columns for everything queried
  or indexed.** Avoids the "fat blob" anti-pattern while keeping schema
  evolution cheap.
- **`ulid` as primary key everywhere** (`TEXT` NOT NULL, 26-char ULID string).
  ULIDs are monotonic-sortable, URL-safe, and more debuggable than raw UUID
  bytes. They replace the current UUID-v4 random IDs and the libSQL integer
  rowids.
- **`tenant_id` on every domain table.** Multi-tenant separation at the data
  layer. `user_id` and `agent_id` are added where semantically meaningful.
  Exception: `brassclaw_process_results` is linked to `brassclaw_processes`
  via foreign key and inherits tenant context from the parent row.
- **`created_at` and `updated_at` on every mutable table.**
  `updated_at` is maintained by a `BEFORE UPDATE` trigger (`set_updated_at()`).
  Each table's migration `CREATE TRIGGER` statement is shown in §4.20.
  `brassclaw_process_results` is insert-only (results are never modified) —
  it has only `created_at`.
- **Soft-deletes via `deleted_at TIMESTAMPTZ` on stateful lifecycle tables.**
  Applies to: `brassclaw_runs`, `brassclaw_session_threads`, `brassclaw_extensions`.
  Append-only tables (`brassclaw_events`, `brassclaw_audit_log`,
  `brassclaw_checkpoints`) have no `deleted_at` — they are pruned by TTL
  (see §4.21 on retention).
- **CHECK constraints on all enumerated columns.** Enumerated TEXT columns
  (status, kind, scope_kind, etc.) carry a `CHECK (col IN (...))` constraint
  to enforce DB-level integrity. See each table definition.
- **Partial indexes for common filters** (e.g., active runs, pending approvals).

### 4.2 Config table

```sql
-- V001__config.sql
CREATE TABLE IF NOT EXISTS brassclaw_config (
    tenant_id   TEXT        NOT NULL,
    key         TEXT        NOT NULL,   -- dot-separated, e.g. "llm.default.provider_id"
    value       TEXT        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, key)
    -- Note: no secondary index on tenant_id alone; the (tenant_id, key) PK
    -- already serves prefix scans for "get all config for a tenant".
);
CREATE TRIGGER brassclaw_config_updated_at
    BEFORE UPDATE ON brassclaw_config
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `config.toml` (`boot.*`, `identity.*`, `policy.*`, `drivers.*`,
`harness.*`, `runner.*`, `skills.*`, `tokens.*`, `webui.*`, `budget.*`,
`trigger_poller.*`, `llm.*`), `sempai_provider.json`.

**Non-string value serialization.** `value` is plain `TEXT`. Non-string config
values in `RebornConfigFile` (booleans, integers, decimals) are serialized as
their natural string representations:
- Booleans: `"true"` / `"false"` (lowercase, matches TOML).
- Integers: decimal string (e.g. `"20"`).
- Floating-point / monetary: decimal string (e.g. `"5.00"`).
- Optional fields absent from config: the row is simply absent (no row with
  value `"null"` or `""`). The `db_config::load_config_snapshot` function uses
  `Option` and a fallback default for every field, matching the existing
  `RebornConfigFile::default()` behaviour.
`db_config::save_config_key` and `load_config_snapshot` are the only places
this serialization contract is enforced. They must be the only callers that
read/write `brassclaw_config` rows — never raw SQL from other modules.

Bootstrap sequence: on first boot, if the table is empty for the tenant, the
CLI first-run wizard writes sensible defaults here (see §6).

**Config live-reload:** `load_config_snapshot` is called once at startup. Live
reload (without restart) is not supported in v1 of this plan. This preserves
the current behaviour where config.toml is also read only at boot.

### 4.3 LLM providers table

```sql
-- V002__llm_providers.sql
CREATE TABLE IF NOT EXISTS brassclaw_llm_providers (
    tenant_id       TEXT        NOT NULL,
    id              TEXT        NOT NULL,   -- provider id, e.g. "openai-custom"
    definition      JSONB       NOT NULL,   -- ProviderDefinition JSON (no api_key values)
    is_builtin      BOOLEAN     NOT NULL DEFAULT false,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ,
    PRIMARY KEY (tenant_id, id)
);
CREATE TRIGGER brassclaw_llm_providers_updated_at
    BEFORE UPDATE ON brassclaw_llm_providers
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `providers.json` and `ProviderRepo`. Built-in providers are never
stored (they are compiled in); only user-overlay providers land here.

### 4.4 Secrets master key

**Headless master-key strategy.** The key-wrapping model depends on the
deployment profile:

- **`local-dev` / `local-dev-yolo` (embedded PG, single user):** The AES-256
  master key is stored *unwrapped* as a 0600-permission file at
  `$REBORN_HOME/.secrets-master-key` — equivalent trust to the current
  `.reborn-local-dev-secrets-master-key` file. No passphrase required.
  A loud warning is printed at startup reminding operators not to use this
  mode for multi-user or internet-facing deployments.

- **`production` (systemd service, `BRASSCLAW_PG_URL` or embedded PG):**
  Production headless boot requires a working unwrap path at **every start**,
  not only at the one-time `rewrap` run. The supported strategies are:

  | `--strategy` value | Wrap-time key source | Per-boot unwrap mechanism | Suitable for headless systemd? |
  |---|---|---|---|
  | `passphrase` | Interactive terminal prompt | **Reads `BRASSCLAW_SECRETS_PASSPHRASE_FILE` at every boot** — operator must save the interactive passphrase to this file | Yes, if the operator also sets `BRASSCLAW_SECRETS_PASSPHRASE_FILE` |
  | `passphrase-file=<path>` | Reads passphrase from specified file at wrap time | Same file re-read at every boot — fully unattended | **Yes — recommended for production** |
  | `keychain` | OS keyring (macOS Keychain / GNOME Keyring) | Requires unlocked keyring at boot | **No** — no D-Bus session under `User=brassclaw` headless systemd |

  > **M2 — `passphrase` vs `passphrase-file` clarification:** `--strategy passphrase`
  > prompts interactively at wrap time. For unattended reboots, the operator must
  > also save that same passphrase into the `BRASSCLAW_SECRETS_PASSPHRASE_FILE`
  > file manually. `--strategy passphrase-file=<path>` is cleaner for production:
  > it reads and saves the passphrase from a file in one step. In practice,
  > **always use `passphrase-file` for production**; `passphrase` is for
  > ad-hoc interactive decryption sessions or local dev.

  **`BRASSCLAW_SECRETS_PASSPHRASE_FILE`** (bootstrap tier, production only) is
  the required unattended-boot path. It is a path to a file containing the
  Argon2id passphrase — **must be readable by the service user** (`brassclaw`),
  not by root only; see §7 for ownership requirements. This var is **not
  deferred to a follow-up**: it is required in §1c (bootstrap tier, production
  only) and in the §7 unit.

  The one-time setup procedure is documented in §7 (fresh install and upgrade
  sequences), which specifies the correct ownership for each file. In brief:
  `rewrap` must run as the service user (`sudo -u brassclaw`) so that
  `master.key` ends up owned `brassclaw:brassclaw 0600` and is readable at
  per-boot unwrap time.

  The service **refuses to boot** in production profile if
  `brassclaw_secrets_master` has no row for the tenant **and** no raw key file
  exists. The raw key file is zeroed and deleted after a successful `rewrap`.

```sql
-- V003__secrets.sql
CREATE TABLE IF NOT EXISTS brassclaw_secrets_master (
    tenant_id       TEXT        NOT NULL,
    version         INT         NOT NULL DEFAULT 1,  -- bumped on rotation
    -- AES-256-GCM key wrapped per the strategy above.
    -- local-dev: wrapped_key = '' AND algorithm = 'raw-key-on-disk'
    --   (key lives at $REBORN_HOME/.secrets-master-key, never in the DB).
    --   The unwrap branch MUST check algorithm = 'raw-key-on-disk' first and
    --   read the key file, NOT attempt to decrypt an empty ciphertext.
    -- production: wrapped_key = base64(nonce || ciphertext), algorithm = 'aes256gcm-argon2id'
    wrapped_key     TEXT        NOT NULL DEFAULT '',
    algorithm       TEXT        NOT NULL DEFAULT 'raw-key-on-disk',
    -- Note: DEFAULT is 'raw-key-on-disk'. The production rewrap command writes
    -- 'aes256gcm-argon2id' explicitly. This prevents a newly-inserted local-dev
    -- row from accidentally using the production algorithm sentinel.
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, version)
);
CREATE TRIGGER brassclaw_secrets_master_updated_at
    BEFORE UPDATE ON brassclaw_secrets_master
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE IF NOT EXISTS brassclaw_secrets (
    tenant_id   TEXT        NOT NULL,
    scope       TEXT        NOT NULL,   -- e.g. "user:alice" or "operator"
    -- name: identifies the credential — e.g. "google_oauth_refresh_token", NOT "OPENAI_API_KEY".
    -- Operator-sourced secrets (API keys, WebUI token) are env-only and never written here.
    -- This table holds only runtime-obtained credentials: OAuth refresh/access tokens
    -- and credential-broker secrets acquired during auth flows (see FilesystemCredentialBroker
    -- in crates/brassclaw_secrets). A breach of this table alone, without master.key /
    -- the passphrase file, cannot expose these credentials.
    name        TEXT        NOT NULL,
    ciphertext  TEXT        NOT NULL,   -- base64(nonce || encrypted value)
    key_version INT         NOT NULL DEFAULT 1,  -- matches brassclaw_secrets_master.version
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, scope, name)
);
CREATE TRIGGER brassclaw_secrets_updated_at
    BEFORE UPDATE ON brassclaw_secrets
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

**`rewrap` vs `rotate` — two distinct operations:**

- **`brassclaw secrets rewrap [--strategy ...] [--tenant <id>]`** — wraps the
  *existing* master key with a new passphrase or strategy. Updates `wrapped_key`
  and `algorithm` on the same `version = 1` row; does **not** generate a new key
  or re-encrypt any `brassclaw_secrets` rows. This is all that §7.1 and §7.2 require.

  **Tenant resolution (data-loss invariant — R6-MH1):** `brassclaw_secrets_master`
  is per-tenant (PK `(tenant_id, version)`). The tenant under which `rewrap`
  writes its row **must** match the `boot_tenant` that §8.1 step 6 will check,
  otherwise step 6 finds no row and aborts — and since `rewrap` already zeroed
  the raw key file, the master key is permanently lost. `rewrap` resolves
  `tenant_id` in this priority order:

  1. **`--tenant <id>` CLI flag** (explicit override — highest priority; use
     this in upgrade runbooks to avoid any ambiguity).
  2. **`identity.tenant` from `$REBORN_HOME/config.toml`** — read via
     `RebornConfigFile::load` (the same loader §8.1 step 3 uses). This is the
     upgrade-path case: `brassclaw_config` is empty when `rewrap` runs manually
     before first serve; config.toml is still present and authoritative.
  3. **`brassclaw_config.identity.tenant`** — read from the DB (the post-migration
     case: config.toml has already been renamed to `.migrated` by a prior run).
  4. **`"default"`** — fallback when neither source yields a value.

  The §7.1 and §7.2 runbook commands pass `--tenant` explicitly to make the
  tenant unambiguous regardless of which files exist on disk. See §7.1 and §7.2
  updated commands below.

  **Key-source rule (data-loss invariant):** `rewrap` MUST read an existing raw
  key file if one is present — it must NOT generate a new key when an existing
  key file exists. Generating a new key when persisted secrets exist orphans
  every ciphertext in `brassclaw_secrets` and `brassclaw_root_filesystem`
  (encrypted under the old key) — those rows become permanently unrecoverable.

  - **Filename search order:** check `$REBORN_HOME/.reborn-local-dev-secrets-master-key`
    first (the pre-migration filename), then `$REBORN_HOME/.secrets-master-key`
    (the post-migration filename). Use whichever is found.
  - **Fresh install (neither file exists and DB is empty):** generate a new key.
  - **Fail-closed:** if neither raw key file is found but `brassclaw_secrets` or
    `brassclaw_root_filesystem` rows are present (would-be orphaned ciphertext),
    `rewrap` must abort with:
    `"raw key file not found but encrypted rows exist — cannot generate new key; restore the original key file first"`.

  **Re-wrapping an already-wrapped key (passphrase change):** when no raw key
  file is present but `brassclaw_secrets_master` already has a row with
  `algorithm = 'aes256gcm-argon2id'` (i.e. the key has already been wrapped
  from a previous `rewrap`), `rewrap` must:
  1. Read `--old-passphrase-file=<path>` if supplied (shell invocation — see note
     below), else `BRASSCLAW_SECRETS_PASSPHRASE_FILE`, else
     `$CREDENTIALS_DIRECTORY/secrets-passphrase` to obtain the *current* (old) passphrase.
  2. Unwrap the stored `wrapped_key` with the old passphrase to recover the
     plaintext AES-256 master key.
  3. Re-wrap with the new `--strategy` and update the `brassclaw_secrets_master`
     row.
  The operator must keep the old passphrase file accessible until `rewrap`
  completes; `rewrap` fails closed if no old passphrase source is available.

  **`--old-passphrase-file=<path>` flag (R6-L1 — shell passphrase-change):**
  `BRASSCLAW_SECRETS_PASSPHRASE_FILE` and `$CREDENTIALS_DIRECTORY` are
  systemd-injected and absent in an interactive shell. When a passphrase change
  is performed interactively (e.g. `sudo -u brassclaw brassclaw secrets rewrap
  --strategy passphrase-file=<new-path>`), the operator must supply the *current*
  passphrase via `--old-passphrase-file=<path>`:
  ```bash
  sudo -u brassclaw brassclaw secrets rewrap \
      --tenant default \
      --strategy passphrase-file=/var/lib/brassclaw/master-new.key \
      --old-passphrase-file=/var/lib/brassclaw/master.key
  ```
  The env-var fallback remains valid for unattended systemd use. Document the
  `--old-passphrase-file` flag and its fallback chain in the §7 passphrase-rotation
  runbook (Phase 9 operator guide).

- **`brassclaw secrets rotate`** (or `rewrap --rotate`) — generates a *new*
  AES-256 master key, inserts a new `version` row into `brassclaw_secrets_master`,
  and re-encrypts all existing `brassclaw_secrets` rows in batches
  (`WHERE key_version < current_version`). `key_version` is what makes this
  incremental and crash-safe. This operation is separate from `rewrap` and
  is not required during upgrade or installation.

  **Old version retirement:** the old `brassclaw_secrets_master` version row
  is deleted **only** after a verification pass confirms no `brassclaw_secrets`
  row has `key_version < new_version`. Deleting it early would make not-yet-
  re-encrypted rows unreadable mid-rotation.

The `§7` sequences only need `rewrap`. Key rotation is a periodic operational
task with its own runbook.

**Legacy raw-key file ↔ `boot_tenant` association (R6-L3):** The legacy raw-key
files (`.reborn-local-dev-secrets-master-key` / `.secrets-master-key`) are single
files with no per-tenant structure — they implicitly belong to `boot_tenant` (the
tenant under which §8.1 step 7 migrates the single-tenant libSQL data). This is
why `rewrap` and §8.1 step 6 must agree on `boot_tenant` (see tenant resolution
note above). Non-`boot_tenant` tenants — created post-migration in a multi-tenant
deployment — have no legacy raw-key file. Their master key is generated fresh on
their first `rewrap` (fresh-install path: neither file exists and DB is empty for
that tenant).

Replaces: `.reborn-local-dev-secrets-master-key` file.

### 4.5 Run state

```sql
-- V004__runs.sql
CREATE TABLE IF NOT EXISTS brassclaw_runs (
    id          TEXT        NOT NULL PRIMARY KEY,   -- ULID
    tenant_id   TEXT        NOT NULL,
    user_id     TEXT        NOT NULL,
    agent_id    TEXT,
    project_id  TEXT,
    thread_id   TEXT,
    status      TEXT        NOT NULL
        CHECK (status IN ('pending','in_progress','completed','failed','stuck')),
    payload     JSONB       NOT NULL DEFAULT '{}',
    started_at  TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at  TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS brassclaw_runs_tenant_status_idx
    ON brassclaw_runs (tenant_id, status) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS brassclaw_runs_thread_idx
    ON brassclaw_runs (tenant_id, thread_id) WHERE thread_id IS NOT NULL;
CREATE TRIGGER brassclaw_runs_updated_at
    BEFORE UPDATE ON brassclaw_runs
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemRunStateStore` / `/runs/*` virtual path.

### 4.6 Approvals

> **Crate note:** `FilesystemApprovalRequestStore` lives in `brassclaw_run_state`;
> a dedicated `brassclaw_approvals` crate also exists. The Pg implementation
> (`PgApprovalRequestStore`) will live in `brassclaw_approvals` (its natural home),
> with `brassclaw_run_state` delegating to it.

```sql
-- V005__approvals.sql
CREATE TABLE IF NOT EXISTS brassclaw_approvals (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    -- ON DELETE RESTRICT: a run must not be hard-deleted while child records
    -- exist. This constraint applies equally to brassclaw_approvals, brassclaw_turns,
    -- and brassclaw_checkpoints (all carry run_id FKs to brassclaw_runs(id) ON DELETE
    -- RESTRICT). Soft-delete (deleted_at) the run instead. If hard-delete is ever
    -- added to PgRunStateStore, it must first DELETE or settle all approval, turn,
    -- and checkpoint rows for that run_id — not just approval rows.
    run_id      TEXT        NOT NULL REFERENCES brassclaw_runs(id) ON DELETE RESTRICT,
    kind        TEXT        NOT NULL
        CHECK (kind IN ('tool_call','shell_command','file_write','network_egress','custom')),
    status      TEXT        NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','approved','denied','expired')),
    request     JSONB       NOT NULL,
    response    JSONB,
    expires_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_approvals_run_idx
    ON brassclaw_approvals (run_id);
CREATE INDEX IF NOT EXISTS brassclaw_approvals_pending_idx
    ON brassclaw_approvals (tenant_id, status) WHERE status = 'pending';
CREATE TRIGGER brassclaw_approvals_updated_at
    BEFORE UPDATE ON brassclaw_approvals
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemApprovalRequestStore` / `/approvals/*`.

### 4.7 Turns

```sql
-- V006__turns.sql
CREATE TABLE IF NOT EXISTS brassclaw_turns (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    -- ON DELETE RESTRICT: see §4.6 comment; hard-delete of a run must settle
    -- all turn rows first.
    run_id      TEXT        NOT NULL REFERENCES brassclaw_runs(id) ON DELETE RESTRICT,
    sequence    INT         NOT NULL,
    status      TEXT        NOT NULL
        CHECK (status IN ('pending','running','completed','failed')),
    payload     JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS brassclaw_turns_run_seq_idx
    ON brassclaw_turns (run_id, sequence);
-- tenant_id index: needed for retention sweeps and any query that lists all
-- turns for a tenant (e.g. "delete all turns for a tenant on offboarding").
-- The MountAlias tenant isolation from FilesystemTurnStateStore was structural
-- (filesystem scoping); the PG store must enforce this via the index and WHERE
-- clauses on tenant_id.
CREATE INDEX IF NOT EXISTS brassclaw_turns_tenant_idx
    ON brassclaw_turns (tenant_id, run_id);
CREATE TRIGGER brassclaw_turns_updated_at
    BEFORE UPDATE ON brassclaw_turns
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemTurnStateStore` / `/turns/*`.

### 4.8 Capability leases

```sql
-- V007__capability_leases.sql
CREATE TABLE IF NOT EXISTS brassclaw_capability_leases (
    id              TEXT        NOT NULL PRIMARY KEY,
    tenant_id       TEXT        NOT NULL,
    user_id         TEXT        NOT NULL,
    capability_id   TEXT        NOT NULL,
    grant           JSONB       NOT NULL,
    expires_at      TIMESTAMPTZ,
    revoked_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_capability_leases_user_cap_idx
    ON brassclaw_capability_leases (tenant_id, user_id, capability_id)
    WHERE revoked_at IS NULL AND (expires_at IS NULL OR expires_at > now());
CREATE TRIGGER brassclaw_capability_leases_updated_at
    BEFORE UPDATE ON brassclaw_capability_leases
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemCapabilityLeaseStore` / `/capabilities/*`.

### 4.9 Session threads

```sql
-- V008__session_threads.sql
CREATE TABLE IF NOT EXISTS brassclaw_session_threads (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    user_id     TEXT        NOT NULL,
    agent_id    TEXT,
    metadata    JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at  TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS brassclaw_session_threads_user_idx
    ON brassclaw_session_threads (tenant_id, user_id) WHERE deleted_at IS NULL;
CREATE TRIGGER brassclaw_session_threads_updated_at
    BEFORE UPDATE ON brassclaw_session_threads
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemSessionThreadService` / `/sessions/*`.

### 4.10 Processes and results

```sql
-- V009__processes.sql
CREATE TABLE IF NOT EXISTS brassclaw_processes (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    run_id      TEXT        REFERENCES brassclaw_runs(id),
    kind        TEXT        NOT NULL
        CHECK (kind IN ('shell','docker','wasm','mcp','custom')),
    status      TEXT        NOT NULL
        CHECK (status IN ('pending','running','completed','failed','cancelled')),
    spec        JSONB       NOT NULL,
    started_at  TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TRIGGER brassclaw_processes_updated_at
    BEFORE UPDATE ON brassclaw_processes
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- insert-only; results are never modified after write
CREATE TABLE IF NOT EXISTS brassclaw_process_results (
    process_id  TEXT        NOT NULL PRIMARY KEY REFERENCES brassclaw_processes(id),
    tenant_id   TEXT        NOT NULL,   -- denormalised for tenant-scoped queries
    exit_code   INT,
    stdout      TEXT,
    stderr      TEXT,
    artifacts   JSONB,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
    -- no updated_at: this table is insert-only
);
```

> **M5 — `tenant_id` integrity:** `brassclaw_process_results.tenant_id` is
> denormalised for performance. No FK or CHECK can enforce it matches the parent
> `brassclaw_processes.tenant_id` without a trigger or deferred constraint.
> **App-layer invariant:** `PgProcessResultStore::insert` must always copy
> `tenant_id` directly from the parent `brassclaw_processes` row (read in the
> same transaction), never from a caller-supplied value. This invariant is
> enforced by the store's write path and covered by an integration test that
> inserts a process + result and asserts both rows carry identical `tenant_id`.

Replaces: `FilesystemProcessStore` / `FilesystemProcessResultStore`.

### 4.11 Extensions

> **Crate note:** `FilesystemExtensionInstallationStore` is a `pub(crate)` type
> in `brassclaw_reborn_composition` (not in `brassclaw_extensions`). The Pg
> implementation (`PgExtensionInstallationStore`) will be added to
> `brassclaw_extensions` (where the trait and `InMemoryExtensionInstallationStore`
> already live), making it a proper crate-public implementation.

```sql
-- V010__extensions.sql
CREATE TABLE IF NOT EXISTS brassclaw_extension_manifests (
    tenant_id   TEXT        NOT NULL,
    name        TEXT        NOT NULL,
    version     TEXT        NOT NULL,
    manifest    JSONB       NOT NULL,   -- parsed from TOML at registration time
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, name, version)
);
CREATE TRIGGER brassclaw_extension_manifests_updated_at
    BEFORE UPDATE ON brassclaw_extension_manifests
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE IF NOT EXISTS brassclaw_extensions (
    id           TEXT        NOT NULL PRIMARY KEY,
    tenant_id    TEXT        NOT NULL,
    user_id      TEXT        NOT NULL,
    name         TEXT        NOT NULL,
    version      TEXT        NOT NULL,
    status       TEXT        NOT NULL DEFAULT 'installed'
        CHECK (status IN ('installed','active','removed')),
    config       JSONB       NOT NULL DEFAULT '{}',
    -- created_at replaces "installed_at" from the draft. §4.1 design philosophy:
    -- "created_at and updated_at on every mutable table." The installation
    -- timestamp IS the created_at for this table. Using a domain-specific name
    -- would violate the consistency rule and make the updated_at trigger naming
    -- inconsistent. Queries that need "installed_at" semantics select created_at.
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    removed_at   TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS brassclaw_extensions_user_idx
    ON brassclaw_extensions (tenant_id, user_id) WHERE removed_at IS NULL;
CREATE TRIGGER brassclaw_extensions_updated_at
    BEFORE UPDATE ON brassclaw_extensions
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemExtensionInstallationStore` + extension manifest TOML files.

### 4.12 Resource accounts (budget / governor)

> **Optimistic locking (CAS):** The existing `FilesystemResourceGovernorStore`
> uses compare-and-swap (`CasSnapshotStore`) — it does NOT use `SELECT FOR UPDATE`.
> The Postgres implementation (`PgResourceGovernorStore`) must replicate this
> behaviour to avoid blocking concurrent budget checks. The `version` column
> enables this: on every budget update, the store issues a conditional UPDATE:
> ```sql
> UPDATE brassclaw_resource_accounts
>    SET reserved = $new_reserved, consumed = $new_consumed,
>        version = version + 1, updated_at = now()
>  WHERE id = $id AND version = $expected_version;
> ```
> If 0 rows are affected (version mismatch → concurrent update), the store
> returns a `BudgetConflict` error and the caller retries (identical to the
> CAS retry loop in `CasSnapshotStore`). The `version` column starts at 0 and
> is incremented on every UPDATE. The `updated_at` trigger still fires (it
> is compatible with this pattern). The `SELECT FOR UPDATE` pattern is
> intentionally **not** used here — it would serialize concurrent budget checks
> and degrade throughput under concurrent agent runs.
>
> **First-write path (R7-M2 fix — no existing row for this period):** The first
> reservation for a `(tenant_id, scope_kind, scope_id, period_key)` tuple has no
> row to read-then-CAS-update. **`INSERT … ON CONFLICT DO UPDATE` must NOT be
> used here** — `excluded.reserved` / `excluded.consumed` are absolute values
> computed by the writer assuming the row was absent. On a conflict, `DO UPDATE SET
> reserved = excluded.reserved` would overwrite the concurrent first-writer's
> reservation with the second writer's stale pre-computed absolute, silently
> losing the first writer's value. This is last-writer-wins, not CAS, and it
> deviates from `CasSnapshotStore` semantics (where the second writer's CAS fails
> and retries with a re-read).
>
> The correct two-step pattern:
> 1. **Ensure the row exists** with a no-op insert:
>    ```sql
>    INSERT INTO brassclaw_resource_accounts
>        (id, tenant_id, scope_kind, scope_id, period_key, reserved, consumed, version)
>    VALUES
>        ($id, $tenant_id, $scope_kind, $scope_id, $period_key, 0, 0, 0)
>    ON CONFLICT (tenant_id, scope_kind, scope_id, period_key) DO NOTHING;
>    ```
>    Whether this INSERT lands or conflicts (concurrent first-writer won), a row
>    now exists with some `version`.
> 2. **Read back** the row: `SELECT reserved, consumed, version … FOR UPDATE` is
>    unnecessary — just read and proceed with the CAS UPDATE:
>    ```sql
>    UPDATE brassclaw_resource_accounts
>       SET reserved = $new_reserved,   -- current_reserved + delta
>           consumed = $new_consumed,
>           version  = version + 1, updated_at = now()
>     WHERE (tenant_id, scope_kind, scope_id, period_key) =
>           ($tenant_id, $scope_kind, $scope_id, $period_key)
>       AND version = $expected_version;
>    ```
>    If 0 rows affected (concurrent writer changed `version` between the read and
>    this UPDATE), return `BudgetConflict` and retry from step 2. This preserves
>    full CAS semantics: both first-writers start from `reserved = 0`, both compute
>    their delta correctly, the second writer's CAS sees `version = 1` (set by the
>    first), retries, reads `reserved = first_delta`, computes
>    `new_reserved = first_delta + second_delta`, and succeeds on the next attempt.

```sql
-- V011__resources.sql
CREATE TABLE IF NOT EXISTS brassclaw_resource_accounts (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    scope_kind  TEXT        NOT NULL
        CHECK (scope_kind IN ('user','project','agent')),
    scope_id    TEXT        NOT NULL,
    period_key  TEXT        NOT NULL,  -- e.g. "2025-01-15" (daily) or "2025-01-W3"
    reserved    NUMERIC(18,6) NOT NULL DEFAULT 0,
    consumed    NUMERIC(18,6) NOT NULL DEFAULT 0,
    limit_usd   NUMERIC(18,6),
    -- version: optimistic locking counter for CAS updates (mirrors CasSnapshotStore
    -- behaviour in FilesystemResourceGovernorStore). Starts at 0; incremented by
    -- every conditional UPDATE. Never reset. See the note above for the UPDATE pattern.
    version     BIGINT      NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, scope_kind, scope_id, period_key)
);
CREATE TRIGGER brassclaw_resource_accounts_updated_at
    BEFORE UPDATE ON brassclaw_resource_accounts
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemResourceGovernorStore` / `/resources/*`.

### 4.13 Checkpoints

> **Crate note:** `FilesystemCheckpointStateStore` lives in `brassclaw_loop_support`
> (not `brassclaw_turns`). Both `InMemoryCheckpointStateStore` and
> `InMemoryLoopCheckpointStore` exist in `brassclaw_turns::checkpoint_state`; the
> latter is not fictitious. The Pg implementations belong in `brassclaw_loop_support`.

```sql
-- V012__checkpoints.sql
-- Retention: keep the last 10 checkpoints per run (enforced by app-layer
-- cleanup after run completion) plus a 30-day TTL background sweep. See §4.21.
-- Note: pg_cron is NOT used — retention runs inside the serve process only (§4.21).
CREATE TABLE IF NOT EXISTS brassclaw_checkpoints (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    -- ON DELETE RESTRICT: see §4.6 comment; hard-delete of a run must settle
    -- all checkpoint rows first.
    run_id      TEXT        NOT NULL REFERENCES brassclaw_runs(id) ON DELETE RESTRICT,
    sequence    INT         NOT NULL,
    payload     BYTEA       NOT NULL,   -- serialised checkpoint blob
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
    -- no updated_at: checkpoints are immutable after write
);
CREATE INDEX IF NOT EXISTS brassclaw_checkpoints_run_idx
    ON brassclaw_checkpoints (run_id, sequence DESC);
CREATE INDEX IF NOT EXISTS brassclaw_checkpoints_tenant_age_idx
    ON brassclaw_checkpoints (tenant_id, created_at);
```

Replaces: `FilesystemCheckpointStateStore` / `/checkpoints/*`.

### 4.14 Events and audit log

> **Retention (H7):** These tables are append-only and grow indefinitely without
> pruning. See §4.21 for the retention/TTL policy. These are operational state,
> not LLM output — CLAUDE.md's "LLM data is never deleted" rule applies to
> conversation history, reasoning, and tool outputs stored in `brassclaw_turns`
> and `brassclaw_checkpoints` payloads, not to the event/audit log rows.

```sql
-- V013__events.sql
-- Retention: 90-day rolling window pruned by a background task (§4.21).
CREATE TABLE IF NOT EXISTS brassclaw_events (
    seq         BIGSERIAL   PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    run_id      TEXT,
    kind        TEXT        NOT NULL,
    payload     JSONB       NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
    -- append-only; no updated_at, no deleted_at
);
CREATE INDEX IF NOT EXISTS brassclaw_events_run_idx
    ON brassclaw_events (run_id) WHERE run_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS brassclaw_events_tenant_idx
    ON brassclaw_events (tenant_id, occurred_at DESC);

-- Retention: 1-year rolling window (compliance-grade; operator-configurable).
CREATE TABLE IF NOT EXISTS brassclaw_audit_log (
    seq         BIGSERIAL   PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    actor_id    TEXT,
    action      TEXT        NOT NULL,
    resource    TEXT,
    payload     JSONB,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
    -- append-only; no updated_at, no deleted_at
);
CREATE INDEX IF NOT EXISTS brassclaw_audit_log_tenant_idx
    ON brassclaw_audit_log (tenant_id, occurred_at DESC);
```

Replaces: `DurableEventLog` + `DurableAuditLog` (both VFS-backed).

### 4.15 Token settings

```sql
-- V014__token_settings.sql
CREATE TABLE IF NOT EXISTS brassclaw_token_settings (
    tenant_id   TEXT        NOT NULL,
    user_id     TEXT        NOT NULL,
    provider_id TEXT        NOT NULL,
    settings    JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, user_id, provider_id)
);
CREATE TRIGGER brassclaw_token_settings_updated_at
    BEFORE UPDATE ON brassclaw_token_settings
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: libSQL `settings` table (`DbTokenSettingsStore`).

### 4.16 Safety config and capability permissions

```sql
-- V015__safety.sql
CREATE TABLE IF NOT EXISTS brassclaw_safety_config (
    id              TEXT        NOT NULL PRIMARY KEY,  -- ULID surrogate key
    tenant_id       TEXT        NOT NULL,
    user_id         TEXT        NOT NULL,
    category        TEXT        NOT NULL,
    pattern         TEXT        NOT NULL,
    is_enabled      BOOLEAN     NOT NULL DEFAULT true,
    is_default      BOOLEAN     NOT NULL DEFAULT false,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Natural-key uniqueness: mirrors the existing INSERT OR IGNORE semantics
    -- (safety_config_store.rs uses (user_id, category, pattern) as the dedup key).
    UNIQUE (tenant_id, user_id, category, pattern)
);
CREATE TRIGGER brassclaw_safety_config_updated_at
    BEFORE UPDATE ON brassclaw_safety_config
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
-- Upsert pattern in PgSafetyConfigStore:
--   INSERT ... ON CONFLICT (tenant_id, user_id, category, pattern) DO NOTHING

CREATE TABLE IF NOT EXISTS brassclaw_capability_permissions (
    tenant_id       TEXT        NOT NULL,
    capability_id   TEXT        NOT NULL,
    permission_mode TEXT        NOT NULL
        CHECK (permission_mode IN ('allow','deny','ask','org_policy')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, capability_id)
);
CREATE TRIGGER brassclaw_capability_permissions_updated_at
    BEFORE UPDATE ON brassclaw_capability_permissions
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: libSQL `safety_config` and `capability_permissions` tables.

### 4.17 Memory docs

```sql
-- V016__memory_docs.sql
CREATE TABLE IF NOT EXISTS brassclaw_memory_docs (
    id               TEXT        NOT NULL,
    tenant_id        TEXT        NOT NULL,
    user_id          TEXT        NOT NULL,
    project_id       TEXT        NOT NULL,
    doc_type         TEXT        NOT NULL,
    title            TEXT        NOT NULL,
    content          TEXT        NOT NULL,
    source_thread_id TEXT,
    tags             TEXT[]      NOT NULL DEFAULT '{}',
    metadata         JSONB       NOT NULL DEFAULT '{}',
    -- Stored generated tsvector column so FTS index stays current on every
    -- INSERT/UPDATE without a separate trigger or manual expression restatement.
    -- PG 12+ (this plan targets PG 16). coalesce guards future nullable columns.
    tsv              tsvector    GENERATED ALWAYS AS (
                         to_tsvector('english',
                             coalesce(title, '') || ' ' || coalesce(content, ''))
                     ) STORED,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, user_id, project_id, id)
);
-- FTS index over the generated column — auto-maintained on every write.
CREATE INDEX IF NOT EXISTS brassclaw_memory_docs_fts_idx
    ON brassclaw_memory_docs USING GIN (tsv);
-- Fast tag lookup
CREATE INDEX IF NOT EXISTS brassclaw_memory_docs_tags_idx
    ON brassclaw_memory_docs USING GIN (tags);
CREATE TRIGGER brassclaw_memory_docs_updated_at
    BEFORE UPDATE ON brassclaw_memory_docs
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: libSQL `memory_docs` table (`MemoryDocLibSqlStore`).
**Improvements over original:** (1) `TEXT[]` tags replaces the JSON-encoded
`tags_json` string; (2) `GENERATED ALWAYS AS ... STORED` tsvector means the GIN
index is auto-maintained on every INSERT/UPDATE — a plain expression GIN index
would have required every FTS query to repeat the identical expression verbatim.

### 4.18 Hook predicate state (verbatim from source)

> **DDL copied verbatim from `crates/brassclaw_hooks_postgres/migrations/V1__predicate_state.sql`.**
> All 8 indexes are preserved. Column order matches the source (`scope_hash` first,
> which the source comment explains is deliberate: "scope_hash is the trust boundary").
> `IF NOT EXISTS` guards added for refinery idempotency (see §3).

```sql
-- V017__hooks.sql
--
-- Design notes (preserved from V1__predicate_state.sql source):
--
-- scope_hash: BYTEA, not TEXT — raw blake3 digest; TEXT would require an
--   extra encoding/decoding step on every read/write and wastes ~35% space.
--   scope_hash is the trust boundary: all queries must scope to it first to
--   prevent cross-tenant predicate leakage.
--
-- key_hash: same BYTEA rationale as scope_hash.
--
-- event_id: TEXT, NOT uuid — event IDs are blake3 64-char hex digests.
--   A UUID column (128-bit) cannot store a 256-bit hex string; attempting to
--   cast would silently truncate and cause phantom dedup failures. Do NOT
--   change this column to UUID — any future migration that does so will reject
--   all existing 64-char hex event IDs. The column stays TEXT.
--
-- occurred_at: window-clock basis for per-key COUNT and LRU eviction queries.
--   Using TIMESTAMPTZ (not BIGINT epoch) allows age-based sweep queries
--   (WHERE occurred_at < now() - interval 'N days') directly without
--   epoch arithmetic. Retention is enforced by the brassclaw serve sweep
--   and brassclaw maintenance prune-old-data (§4.21) — not pg_cron.
--
CREATE TABLE IF NOT EXISTS hooks_predicate_invocations (
    scope_hash   BYTEA       NOT NULL,
    key_hash     BYTEA       NOT NULL,
    event_id     TEXT        NOT NULL,
    occurred_at  TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (key_hash, event_id)
);
-- Per-key window-trim + COUNT scan
CREATE INDEX IF NOT EXISTS hooks_predicate_invocations_key_ts_idx
    ON hooks_predicate_invocations (key_hash, occurred_at);
-- Per-scope (tenant) distinct-key LRU eviction scans
CREATE INDEX IF NOT EXISTS hooks_predicate_invocations_scope_idx
    ON hooks_predicate_invocations (scope_hash);
-- Per-tenant LRU quota composite
CREATE INDEX IF NOT EXISTS hooks_predicate_invocations_scope_key_idx
    ON hooks_predicate_invocations (scope_hash, key_hash);
-- Operator reaper (evict_older_than) by age
CREATE INDEX IF NOT EXISTS hooks_predicate_invocations_ts_idx
    ON hooks_predicate_invocations (occurred_at);

CREATE TABLE IF NOT EXISTS hooks_predicate_values (
    scope_hash   BYTEA       NOT NULL,
    key_hash     BYTEA       NOT NULL,
    event_id     TEXT        NOT NULL,
    occurred_at  TIMESTAMPTZ NOT NULL,
    value        NUMERIC     NOT NULL,
    PRIMARY KEY (key_hash, event_id)
);
CREATE INDEX IF NOT EXISTS hooks_predicate_values_key_ts_idx
    ON hooks_predicate_values (key_hash, occurred_at);
CREATE INDEX IF NOT EXISTS hooks_predicate_values_scope_idx
    ON hooks_predicate_values (scope_hash);
CREATE INDEX IF NOT EXISTS hooks_predicate_values_scope_key_idx
    ON hooks_predicate_values (scope_hash, key_hash);
CREATE INDEX IF NOT EXISTS hooks_predicate_values_ts_idx
    ON hooks_predicate_values (occurred_at);
```

Note: `hooks_*` tables intentionally keep no `brassclaw_` prefix for backward
compatibility with existing deployments that already have these tables.
These tables are append-only (pruned by `evict_older_than`); no `updated_at`
trigger is needed.

No semantic schema change vs. the source. Migration consolidates it here so
`brassclaw_pg` is the single migration authority.

### 4.19 Root filesystem fallback

> **Multi-tenant isolation gap fixed (C2):** The earlier draft had no `tenant_id`
> on `brassclaw_root_filesystem`. This is dangerous: §4.4 (`rewrap`) queries this
> table for encrypted rows (`WHERE contents IS NOT NULL AND kind = 'encrypted'`) to
> determine whether a new key can be safely generated. Without `tenant_id`, this
> check is global — it would find encrypted rows from other tenants and block a
> per-tenant `rewrap`. Worse, a `PostgresRootFilesystem` implementation scoped to
> one tenant could accidentally surface another tenant's encrypted blobs if the
> query is not properly scoped. **`tenant_id` is required.** The PK changes from
> `(path)` to `(tenant_id, path)` to enforce per-tenant path isolation.

```sql
-- V018__root_filesystem.sql  (kept for unrecognised VFS paths)
CREATE TABLE IF NOT EXISTS brassclaw_root_filesystem (
    -- tenant_id is required for multi-tenant isolation. The rewrap encrypted-row
    -- check (§4.4 fail-closed rule) scopes to tenant_id to avoid cross-tenant
    -- false positives. PostgresRootFilesystem must always scope queries to tenant_id.
    tenant_id   TEXT        NOT NULL,
    path        TEXT        NOT NULL,
    contents    BYTEA,
    is_dir      BOOLEAN     NOT NULL DEFAULT false,
    content_type TEXT,
    kind        TEXT,
    indexed     JSONB,
    version     BIGINT      NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, path)
);
CREATE INDEX IF NOT EXISTS brassclaw_root_filesystem_tenant_encrypted_idx
    ON brassclaw_root_filesystem (tenant_id)
    WHERE contents IS NOT NULL AND kind = 'encrypted';
CREATE TRIGGER brassclaw_root_filesystem_updated_at
    BEFORE UPDATE ON brassclaw_root_filesystem
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

This is a slim fallback for any VFS path not covered by a domain table.
Long-term goal: eliminate it entirely by migrating each remaining path to a
typed table.

### 4.20 Shared trigger: updated_at

`V000__shared_triggers.sql` runs first and defines the function only.
Each table's own migration includes its individual `CREATE TRIGGER` statement
(shown in §4.2–§4.19 above). This keeps each migration self-contained and
makes it clear which tables have the trigger without requiring implementers to
cross-reference V000.

```sql
-- V000__shared_triggers.sql  (runs first — defines the function only)
CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at := now();
    RETURN NEW;
END;
$$;
```

### 4.21 Retention / TTL policy

Append-only tables grow indefinitely without operator action. The following
default retention windows apply; all are configurable via `brassclaw_config`
keys (`retention.*`):

| Table | Default retention | Enforcement |
|---|---|---|
| `brassclaw_checkpoints` | Last 10 per run + 30 days | App-layer cleanup after run completion; background sweep daily |
| `brassclaw_events` | 90 days | Background task pruning `WHERE occurred_at < now() - interval '90 days'` |
| `brassclaw_audit_log` | 1 year | Background task (operator may extend for compliance) |
| `brassclaw_runs` (soft-deleted) | 90 days after `deleted_at` | Background sweep |
| `brassclaw_extensions` (removed) | 90 days after `removed_at` | Background sweep |

Pruning tasks run as part of a background maintenance loop inside the
**`brassclaw serve` process only** (not via `pg_cron`, to avoid an external
dependency). `brassclaw run` (one-shot CLI) does not start the maintenance
loop and performs no retention sweep on exit. Consequence: operators who use
`brassclaw run` exclusively (no long-running serve) will see unbounded growth
in `brassclaw_checkpoints` and `brassclaw_events` until they run
`brassclaw serve` at least once (or manually run
`brassclaw maintenance prune-old-data`). This is documented in the operator
guide produced in Phase 9.

CLAUDE.md's "LLM data is never deleted" rule is not violated: LLM output
(reasoning, tool calls, messages) lives in `brassclaw_turns.payload` and
`brassclaw_checkpoints.payload`, which are retained until the run itself is
soft-deleted past its TTL.

---

## 5. Crate Changes

### 5.1 `brassclaw_filesystem`

- The `postgres` and `libsql` feature gates are removed.
- The `LibSqlRootFilesystem`, `LibSqlBackend`, `InMemoryBackend` concrete types
  are replaced by a single `PostgresRootFilesystem` backed by
  `brassclaw_root_filesystem` table (for any VFS path not yet promoted to a
  domain table).
- The `RootFilesystem` and `ScopedFilesystem` traits stay unchanged.
- Domain-specific Filesystem stores (`FilesystemRunStateStore`, etc.) are
  replaced one-by-one by Postgres-native store types. See §5.3 below.

### 5.2 `brassclaw_hooks_postgres` and `brassclaw_hooks_libsql`

- `brassclaw_hooks_libsql` is **deleted entirely** — the crate is removed from
  the workspace.
- `brassclaw_hooks_postgres` loses its `postgres` optional feature gate.
  `deadpool-postgres` and `tokio-postgres` become mandatory deps (no longer
  optional). The crate is renamed to `brassclaw_hooks_pg` for clarity.
- `brassclaw_hooks_parity` (cross-backend parity tests) is deleted; the
  `brassclaw_hooks_pg` contract test suite is the only test.

### 5.3 Store crates: replaced implementations

Each of these stores gains a `brassclaw_*_pg` module implementing the same
public trait against Postgres via `brassclaw_pg::PgPool`:

Verified source locations for every old type (via `grep -rn "pub struct Filesystem.*Store"`):

| Old type | Source crate (verified) | New type | New crate |
|---|---|---|---|
| `FilesystemRunStateStore` | `brassclaw_run_state` (lib.rs:555) | `PgRunStateStore` | `brassclaw_run_state` |
| `FilesystemApprovalRequestStore` | `brassclaw_run_state` (lib.rs:787) | `PgApprovalRequestStore` | `brassclaw_approvals` ⬡ |
| `FilesystemTurnStateStore` | `brassclaw_turns` (filesystem_store.rs:80) | `PgTurnStateStore` | `brassclaw_turns` |
| `FilesystemCheckpointStateStore` | `brassclaw_loop_support` (filesystem_checkpoint_state.rs:44) | `PgCheckpointStateStore` | `brassclaw_loop_support` ⬡ |
| `InMemoryCheckpointStateStore` | `brassclaw_turns` (checkpoint_state.rs:216) | `PgCheckpointStateStore` (shared) | `brassclaw_loop_support` |
| `InMemoryLoopCheckpointStore` | `brassclaw_turns` (checkpoint_state.rs:221) | `PgLoopCheckpointStore` | `brassclaw_turns` |
| `FilesystemCapabilityLeaseStore` | `brassclaw_authorization` (lib.rs:417) | `PgCapabilityLeaseStore` | `brassclaw_authorization` |
| `FilesystemSessionThreadService` | `brassclaw_threads` (filesystem_service.rs:150) | `PgSessionThreadService` | `brassclaw_threads` |
| `FilesystemResourceGovernorStore` | `brassclaw_resources` (filesystem_store.rs:68) | `PgResourceGovernorStore` | `brassclaw_resources` |
| `FilesystemProcessStore` | `brassclaw_processes` (filesystem_store.rs:54) | `PgProcessStore` | `brassclaw_processes` |
| `FilesystemProcessResultStore` | `brassclaw_processes` (filesystem_store.rs:415) | `PgProcessResultStore` | `brassclaw_processes` |
| `FilesystemExtensionInstallationStore` | `brassclaw_reborn_composition` (extension_installation_store.rs:14, `pub(crate)`) | `PgExtensionInstallationStore` | `brassclaw_extensions` ⬡ |
| `FilesystemDurableEventLog` | `brassclaw_events` | `PgDurableEventLog` | `brassclaw_events` |
| `FilesystemDurableAuditLog` | `brassclaw_events` | `PgDurableAuditLog` | `brassclaw_events` |
| `DbTokenSettingsStore` | `brassclaw_reborn_composition` | `PgTokenSettingsStore` | `brassclaw_reborn_composition` |
| `SqliteSafetyConfigStore` | `brassclaw_product_workflow` | `PgSafetyConfigStore` | `brassclaw_product_workflow` |
| `MemoryDocLibSqlStore` | `brassclaw_reborn_composition` | `PgMemoryDocStore` | `brassclaw_reborn_composition` |
| `FilesystemAuthProductServices` | `brassclaw_reborn_composition` | `PgAuthProductServices` | `brassclaw_reborn_composition` |
| `FilesystemCredentialBroker` | `brassclaw_secrets` | `PgCredentialBroker` | `brassclaw_secrets` |
| `FilesystemSecretStore` | `brassclaw_secrets` | `PgSecretStore` | `brassclaw_secrets` |

**⬡ Relocation notes:**
- `PgApprovalRequestStore` moves to `brassclaw_approvals` (the dedicated crate where the
  trait and domain logic belong); `brassclaw_run_state` will delegate to it.
- `PgCheckpointStateStore` moves to `brassclaw_loop_support` (where `FilesystemCheckpointStateStore`
  actually lives, not `brassclaw_turns` as the original plan stated).
- `PgExtensionInstallationStore` moves from `brassclaw_reborn_composition` (where the
  `pub(crate)` filesystem impl lives) into `brassclaw_extensions` (where the trait and
  `InMemoryExtensionInstallationStore` already live), making it properly crate-public.

The `In-Memory*` variants are kept behind `#[cfg(test)]` only, used in
unit tests that do not need a live database.

### 5.4 `brassclaw_reborn_config`

`brassclaw_reborn_config` remains a **pure, synchronous, no-workspace-deps boundary
crate** — the `reborn_dependency_boundaries.rs` architecture test is not changed.
DB access must not be added here.

- `config_file.rs` — `RebornConfigFile` is retained as a **parse/serialize type only**
  for the long-term serve path. **`load()` is NOT removed in Phase 8** — it is retained
  behind the `migrate-from-libsql` feature flag, because two upgrade-path call sites
  depend on reading `config.toml` from disk at a point when `brassclaw_config` rows do
  not yet exist:
  - **§4.4 `rewrap` tenant-resolution step 2**: resolves `boot_tenant` from
    `config.toml` during manual pre-serve upgrade invocation.
  - **§8.1 step 3**: parses `config.toml` to migrate it into `brassclaw_config` rows.
  At that point `db_config::load_config_snapshot` cannot substitute — it reads from
  `brassclaw_config` rows that are still being written. `load()` removal lands in the
  **next release**, together with the `migrate-from-libsql` feature removal (see §9.1
  and Phase 8 checklist). Runtime serve-path callers are replaced by
  `db_config::load_config_snapshot` in Phase 2 as planned.
- `home.rs` — `RebornHome::config_file_path()`, `providers_file_path()`,
  `sempai_provider_file_path()` are removed. `path()` is kept.
- `secrets_guard.rs` — Kept unchanged.
- `Cargo.toml` — Remove `toml_edit`, `fs4`, `tempfile`. **Keep `toml`** (required
  by `config export` and `config show-all` serialization in §6.3). Keep `serde`.
  Do **not** add `deadpool-postgres` here.

### 5.5 `brassclaw_reborn_composition`

- **New file: `db_config.rs`** — DB-backed config read/write. This is where
  the DB access lives (not in `brassclaw_reborn_config`). Public API:
  ```rust
  pub async fn load_config_snapshot(pool: &PgPool, tenant_id: &str) -> Result<RebornConfigFile>;
  pub async fn save_config_key(
      pool: &PgPool,
      tenant_id: &str,
      key: &str,
      value: &str,
      caller: ConfigWriteContext,
  ) -> Result<()>;
  ```
  where `ConfigWriteContext` is:
  ```rust
  pub enum ConfigWriteContext { Operator, AgentSession }
  ```
  `save_config_key` rejects any `key` ending in `_env` when
  `caller == ConfigWriteContext::AgentSession`, returning
  `ConfigError::EnvKeyWriteForbidden { key }` (see §1c security note on
  agent write-gate). This is structural: no caller that holds only a pool
  handle and a session context can reroute which env variable the serve
  process reads for auth or identity. Assembles a `RebornConfigFile` from
  rows and hands it to the composition layer.
- `provider_repo.rs` — `ProviderRepo` rewritten to read/write `brassclaw_llm_providers`
  instead of `providers.json`.
- `llm_config_service.rs` — Updated to call `db_config::load_config_snapshot`
  and the DB-backed `ProviderRepo`; file paths removed.
- `llm_key_store.rs` — `LlmKeyStore` already wraps the secret store; unchanged.
- `llm_catalog.rs` — `resolve_against_registry` now loads providers from
  `brassclaw_llm_providers` at start instead of reading `providers.json`.
- `factory.rs` — Simplified dramatically:
  - The libSQL bundle (`LocalDevRootFilesystemBundle`, `libsql::Database` arc)
    is removed.
  - `build_local_dev_root_filesystem` is removed.
  - All `#[cfg(feature = "libsql")]` / `#[cfg(feature = "postgres")]` guards
    are removed — there is only one path now.
  - The Postgres pool is built from `brassclaw_embedded_postgres::ManagedPostgres`
    or `BRASSCLAW_PG_URL`, then passed to every store constructor.
  - `RebornLocalRuntimeServices` loses the `identity_substrate_db` and all
    libSQL-specific fields.
  - Pool drop happens before `managed_pg.shutdown().await` is called.
- `hooks/` — Any libSQL hook backend wiring is removed.

---

## 6. Onboarding / First-Run Wizard

The current `config init` command (which writes `config.toml` + `providers.json`)
is replaced by an interactive first-run wizard that writes to Postgres.

### 6.1 Trigger

First-run is detected by querying `brassclaw_config` for the tenant's
`boot.initialized = true` key. If absent and stdin is a TTY, the wizard runs
automatically the first time `brassclaw serve` or `brassclaw run` is called.

**Non-interactive guard:** If `boot.initialized` is absent and stdin is **not**
a TTY (e.g. a systemd service), `brassclaw serve` must **not** launch the
interactive wizard — it must fail immediately with a clear, non-zero exit:

```
brassclaw: first-run setup required. Run 'brassclaw config init' before starting the service.
```

This prevents the Restart=on-failure loop that would otherwise occur when the
interactive wizard hangs with no TTY.

### 6.2 Wizard steps (CLI, interactive, all skippable with `--yes`)

```
┌─ BrassClaw First-Run Setup ───────────────────────────────────────┐
│                                                                    │
│  Step 1/5  LLM Provider                                            │
│    Choose a provider: [openai / anthropic / ollama / custom / skip]│
│    Model [gpt-4o-mini]:                                            │
│    API key env var name [OPENAI_API_KEY]:                          │
│    (Only the env var NAME is stored here.                          │
│     The value is read from the environment at runtime —            │
│     set it in the systemd unit or your shell profile.)             │
│                                                                    │
│  Step 2/5  WebUI Access                                            │
│    Bearer token env var name [BRASSCLAW_REBORN_WEBUI_TOKEN]:       │
│    (Only the env var NAME is stored. Set the value in the          │
│     secrets file — see §7.)                                        │
│    WebUI user-id env var name [BRASSCLAW_REBORN_WEBUI_USER_ID]:    │
│    (brassclaw serve hard-errors if this env var is unset at start) │
│                                                                    │
│  Step 3/5  Identity                                                │
│    Tenant ID [default]:                                            │
│    Default owner ID [admin]:                                       │
│    (Stored in brassclaw_config as identity.default_owner —        │
│     this is a CONFIG value used for new session defaults.          │
│     It is separate from the BRASSCLAW_REBORN_WEBUI_USER_ID env    │
│     var set in Step 2, which is the identity asserted for bearer   │
│     auth at serve time. They should match in single-user setups;   │
│     the wizard warns if they differ.)                              │
│                                                                    │
│  Step 4/5  Budget                                                  │
│    Daily user budget in USD [5.00]:                                │
│    (0 = unlimited)                                                 │
│                                                                    │
│  Step 5/5  SSO (optional — skip if using bearer-token auth only)   │
│    WebUI base URL [skip]:                                          │
│    Allowed email domains (comma-separated) [skip]:                 │
│    (Non-secrets: stored in brassclaw_config, no env var needed.)   │
│                                                                    │
│  Writing to PostgreSQL...  ✓                                       │
│  Run `brassclaw serve` to start.                                   │
└────────────────────────────────────────────────────────────────────┘
```

The wizard writes each answer as a `brassclaw_config` row, then sets
`boot.initialized = true`. It never writes a file. API key values are
always read at runtime from the env var named in the config (env-only by
security policy, consistent with §1b–§1c).

### 6.3 `brassclaw config` subcommands (replaces file editing)

```
brassclaw config get <key>
brassclaw config set <key> <value>
brassclaw config list [--section <section>]
brassclaw config unset <key>
brassclaw config show-all          # prints all config as TOML for inspection
brassclaw config export > config.toml   # export to file for backup
brassclaw config import < config.toml  # import from file
```

`config show-all` renders the DB rows back into the `RebornConfigFile` shape,
making existing operator documentation still applicable for reference.

### 6.4 CLI command Postgres lifecycle

Every DB-touching CLI command — `config init`, `secrets rewrap`,
`config set/get/list/unset/show-all/export/import`, `maintenance prune-old-data`
— follows this lifecycle:

1. Start embedded Postgres **or connect to an already-running instance.** Uses
   the §2.2 orphaned-server detection: check `postmaster.pid` PID liveness
   (kill -0). If a live postmaster is found, reuse it — do not start a second
   instance. Record whether this command started PG or merely connected to an
   existing one.
2. Run `brassclaw_pg::migrations::run_migrations` — **idempotent** (all DDL
   uses `CREATE TABLE IF NOT EXISTS`; the §3 history-reconciliation bootstrap
   ensures pre-existing `hooks_*` and legacy tables don't trip refinery).
3. Perform the operation.
4. **Conditional shutdown:** shut down embedded PG **only if this command
   started it** (step 1 found no live postmaster). If a running PG was detected
   and reused (e.g. `brassclaw serve` is already running and owns the embedded
   PG), leave it running — shutting it down would crash the live server. For
   external PG (`BRASSCLAW_PG_URL`), always release the pool connection (no
   process to shut down).

Only `brassclaw serve` keeps Postgres running for the entire process lifetime
without ever triggering the conditional shutdown.

**This is why `secrets rewrap` can write `brassclaw_secrets_master` before
`brassclaw serve` has ever started**, and why `§8.1 step 6` can rely on that
row already existing at serve time: `rewrap` ran the schema first in step 2.
It is also why `brassclaw config init` in `§7.1 step 1` starts embedded PG
automatically and runs migrations before writing `brassclaw_config`.

### 6.5 `brassclaw config init --yes` flag mapping

The `--yes` flag bypasses all interactive prompts and applies the flag
defaults. Each wizard step maps to one or more flags:

| Wizard step | Flag(s) | Default |
|---|---|---|
| Step 1 — LLM provider | `--provider <id>` or `--no-llm` | required; use `--no-llm` to skip LLM setup |
| Step 1 — LLM model | `--model <name>` | required when `--provider` is set |
| Step 1 — API key env var | `--api-key-env <VAR>` | Provider-specific default: `openai` → `OPENAI_API_KEY`; `anthropic` → `ANTHROPIC_API_KEY`; `ollama` → *(none, no key needed)*; any other provider → required |
| Step 2 — WebUI token env var | `--webui-token-env <VAR>` | `BRASSCLAW_REBORN_WEBUI_TOKEN` |
| Step 2 — WebUI user-id env var | `--webui-user-id-env <VAR>` | `BRASSCLAW_REBORN_WEBUI_USER_ID` |
| Step 3 — Tenant ID | `--tenant <id>` | `default` |
| Step 3 — Default owner ID | `--owner <id>` | `admin` |
| Step 4 — Daily budget (USD) | `--budget-usd <n>` | `5.00` |
| Step 5 — WebUI base URL | `--webui-base-url <url>` | *(skipped)* |
| Step 5 — Allowed email domains | `--webui-allowed-domains <list>` | *(skipped)* |

When `--yes` is given and a required flag is omitted, `config init` exits with
a clear error (no interactive fallback). Running `config init --yes` with all
required flags is idempotent (upsert behaviour) — safe to re-run.

---

## 7. Systemd Service File

> **This is a template.** Operators must complete the `EnvironmentFile=` before
> starting the service. See the sequences below for fresh-install vs upgrade.
>
> **File ownership rules:**
> - `secrets.env` is read by **systemd** (as root) — `root:root 0600` is correct.
> - `master.key` (the Argon2id passphrase) is opened by the **service process**
>   (`User=brassclaw`) at per-boot unwrap time — it must be `brassclaw:brassclaw 0600`
>   (or `root:brassclaw 0640`). If the file is `root:root 0600` the service gets
>   `EACCES` and boot fails. See C2 fix in the setup sequences below.
> - The embedded PG data directory (`$REBORN_HOME/postgres/data/`) is created by
>   `initdb` which hard-refuses to run as root. All setup commands that touch the
>   data dir or `master.key` must run as `brassclaw`, not as root.
>
> The env var names in `secrets.env` (e.g. `OPENAI_API_KEY=sk-...`) must match the
> `api_key_env` values set in `brassclaw_config` during `config init`. A hardcoded
> `OPENAI_API_KEY` line is correct only when the operator kept the default name.

### 7.0 Prerequisites (fresh host)

Before running §7.1 or §7.2, ensure the service user, directories, and binary
are in place. On a fresh Debian/Ubuntu-family host:

```bash
# 1. Create the service user (no home-dir login, no shell):
useradd -r -d /var/lib/brassclaw -s /usr/sbin/nologin brassclaw  # Debian/Ubuntu
# On RHEL/Fedora, use: useradd -r -d /var/lib/brassclaw -s /sbin/nologin brassclaw

# 2. Create the required directories:
install -d -m 0750 -o brassclaw -g brassclaw /var/lib/brassclaw
install -d -m 0750 -o root      -g root      /etc/brassclaw
install -d -m 0755 -o root      -g root      /opt/brassclaw

# 3. Install the binary:
install -m 0755 ./target/release/brassclaw /usr/local/bin/brassclaw
```

The Phase 9 operator guide must include these steps.

### 7.1 Fresh-install sequence (no prior BrassClaw state)

```bash
# All commands that touch the data dir or master.key MUST run as the service user.
# initdb refuses root; master.key must be brassclaw-readable at per-boot unwrap.

# 1. Write initial config as the service user.
#    config init starts embedded PG, runs schema migrations (idempotent — see §6.4),
#    then writes brassclaw_config rows and sets boot.initialized = true:
sudo -u brassclaw brassclaw config init --yes \
    --provider openai --model gpt-4o-mini \
    --api-key-env OPENAI_API_KEY \
    --owner admin --tenant default \
    --budget-usd 5.00

# 2. Wrap master key as the service user (so master.key → brassclaw:brassclaw 0600).
#    --tenant must match the --tenant passed to config init above (§4.4 tenant-
#    resolution rule: explicit flag beats config.toml lookup, avoids any ambiguity):
sudo -u brassclaw brassclaw secrets rewrap \
    --tenant default \
    --strategy passphrase-file=/var/lib/brassclaw/master.key

# 3. Populate secrets.env — read by systemd as root, so root:root 0600 is correct:
install -m 0600 /dev/null /etc/brassclaw/secrets.env
# Path to brassclaw-readable passphrase file (set in step 2):
echo "BRASSCLAW_SECRETS_PASSPHRASE_FILE=/var/lib/brassclaw/master.key" >> /etc/brassclaw/secrets.env
echo "BRASSCLAW_REBORN_WEBUI_TOKEN=your-bearer-token"                  >> /etc/brassclaw/secrets.env
echo "BRASSCLAW_REBORN_WEBUI_USER_ID=admin"                            >> /etc/brassclaw/secrets.env
echo "OPENAI_API_KEY=sk-..."                                            >> /etc/brassclaw/secrets.env

# 4. Start the service:
systemctl start brassclaw
```

### 7.2 Upgrade-from-file/libSQL sequence (existing BrassClaw installation)

> **Do NOT run `brassclaw config init`** on an upgrade — §8.1 step 3 migrates
> `config.toml` and step 7 migrates `reborn-local-dev.db` automatically on first
> serve. Running `config init --yes` would clobber the migrated config with
> defaults.
>
> `rewrap` starts embedded PG and runs schema migrations (§6.4), so
> `brassclaw_secrets_master` exists before `brassclaw serve` ever starts.
> §8.1 step 6 therefore finds the row created by `rewrap` and does not exit
> non-zero.
>
> **Existing persisted secrets:** any OAuth tokens or credential-broker secrets
> stored in the old libSQL DB are encrypted under the existing master key at
> `.reborn-local-dev-secrets-master-key`. `rewrap` reads that file (see the
> key-source rule in §4.4) and wraps *the same key* — so migrated secrets remain
> decryptable after §8.1 step 7 migrates the rows to PG. `rewrap` then zeroes
> and deletes the old file, preventing §8.1 step 6 from aborting on first serve.

```bash
# 1. Wrap the master key as the service user.
#    rewrap reads .reborn-local-dev-secrets-master-key (preserving decryptability
#    of migrated persisted secrets), runs schema migrations, writes a
#    brassclaw_secrets_master row, then zeroes/deletes the old key file.
#
#    IMPORTANT: --tenant must match identity.tenant in config.toml (§4.4
#    tenant-resolution rule). Passing --tenant explicitly avoids the risk of
#    rewrap defaulting to "default" when config.toml has a different value.
#    config.toml uses TOML section syntax ([identity] + tenant = "..."), not
#    dot-notation, so grep for the key inside the section:
#    grep tenant $BRASSCLAW_REBORN_HOME/config.toml
sudo -u brassclaw brassclaw secrets rewrap \
    --tenant <identity.tenant from config.toml> \
    --strategy passphrase-file=/var/lib/brassclaw/master.key

# 2. Populate secrets.env (same as fresh install):
install -m 0600 /dev/null /etc/brassclaw/secrets.env
echo "BRASSCLAW_SECRETS_PASSPHRASE_FILE=/var/lib/brassclaw/master.key" >> /etc/brassclaw/secrets.env
echo "BRASSCLAW_REBORN_WEBUI_TOKEN=your-bearer-token"                  >> /etc/brassclaw/secrets.env
echo "BRASSCLAW_REBORN_WEBUI_USER_ID=admin"                            >> /etc/brassclaw/secrets.env
echo "OPENAI_API_KEY=sk-..."                                            >> /etc/brassclaw/secrets.env

# 3. Start the service — §8.1 migration runs automatically on first serve:
systemctl start brassclaw
```

### 7.3 Service unit file

```ini
# /etc/systemd/system/brassclaw.service
[Unit]
Description=BrassClaw Reborn Agent
After=network.target

[Service]
Type=simple
User=brassclaw
WorkingDirectory=/opt/brassclaw

# Bootstrap tier — non-secret; safe as inline Environment=
Environment=BRASSCLAW_REBORN_HOME=/var/lib/brassclaw
Environment=BRASSCLAW_REBORN_PROFILE=production
Environment=BRASSCLAW_REBORN_LOG=brassclaw=info
# Optional — omit to use embedded Postgres:
# Environment=BRASSCLAW_PG_URL=postgresql://brassclaw@127.0.0.1:5432/brassclaw
# Optional — override embedded PG port if 5434 is taken:
# Environment=BRASSCLAW_EMBEDDED_PG_PORT=5435

# Operator-trusted tier (secrets + identity values, never inline) — read by
# systemd as root. File must be root:root 0600.
# Contents: BRASSCLAW_SECRETS_PASSPHRASE_FILE (path to brassclaw-readable file),
# BRASSCLAW_REBORN_WEBUI_TOKEN, BRASSCLAW_REBORN_WEBUI_USER_ID, API keys.
EnvironmentFile=/etc/brassclaw/secrets.env

ExecStart=/usr/local/bin/brassclaw serve
Restart=on-failure
RestartSec=5

# Hardening — AGENTS.md: "Review any change touching listeners, auth, secrets with
# a security mindset." Running an embedded DB server requires appropriate isolation.
# NOTE: MemoryDenyWriteExecute=yes requires jit=off in postgresql.conf (§2.2).
# If you remove MDWE, also remove jit=off — see §2.2 for the tandem-change note.
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
# AF_INET covers TCP to 127.0.0.1:5434 (embedded PG); AF_UNIX covers PG unix sockets if used.
RestrictNamespaces=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
# SystemCallFilter=@system-service covers the baseline. PostgreSQL may need
# additional syscalls (clone, mmap, semget, setrlimit, ioprio_*). If the
# hardened-unit integration test (Phase 10) shows PG requires a syscall outside
# @system-service, extend with that specific syscall rather than weakening the
# filter globally (e.g. SystemCallFilter=@system-service semget).
SystemCallFilter=@system-service
# /etc/brassclaw is read-only to the service (ProtectSystem=strict covers it).
# secrets.env is read by systemd-manager (root) via EnvironmentFile= and
# injected as environment — the service process never opens /etc/brassclaw.
ReadWritePaths=/var/lib/brassclaw
CapabilityBoundingSet=
AmbientCapabilities=
LimitNOFILE=4096
TasksMax=512

[Install]
WantedBy=multi-user.target
```

All other configuration (LLM provider id, model, WebUI settings, budget, etc.)
is in the DB, set via `brassclaw config set` or the first-run wizard.

---

## 8. Migration from Existing State

The migration runs automatically on the first boot after upgrading. It is
implemented in `brassclaw_reborn_composition::migration` and runs before
any request is served.

### 8.1 Migration order

> **`migrate-from-libsql` feature gate (R8-L1):** Steps 3–7 are all compiled
> behind the `migrate-from-libsql` feature flag. They depend on
> `RebornConfigFile::load()` (§5.4) and file/libSQL I/O that are removed in
> the next release alongside the feature. The entire
> `brassclaw_reborn_composition::migration` module should be
> `#[cfg(feature = "migrate-from-libsql")]`. Steps 1–2 are unconditional
> (schema migrations run on every boot regardless of upgrade state).

1. Start embedded Postgres (or connect to external).
2. Run all schema migrations (`brassclaw_pg::migrations::run_migrations`),
   including the history-reconciliation bootstrap (§3).
3. **Migrate `config.toml`:** If the file exists, parse it with
   `RebornConfigFile::load`, translate each field to a `(key, value)` pair,
   insert into `brassclaw_config`. Rename the file to `config.toml.migrated`.
   Record that migration occurred.
4. **Migrate `providers.json`:** If the file exists, parse it, upsert each
   `ProviderDefinition` into `brassclaw_llm_providers`. Rename to
   `providers.json.migrated`. Record that migration occurred.
5. **Migrate `sempai_provider.json`:** If the file exists, parse it, write
   `sempai.provider_id` and `sempai.model` into `brassclaw_config`. Rename.
   Record that migration occurred.
6. **Migrate secrets master key:** Handle per deployment profile:
   - *`local-dev` profile:* If `.reborn-local-dev-secrets-master-key` exists,
     copy it to `$REBORN_HOME/.secrets-master-key` (0600), then upsert the
     `brassclaw_secrets_master` row:
     ```sql
     INSERT INTO brassclaw_secrets_master (tenant_id, version, wrapped_key, algorithm)
     VALUES (<boot_tenant>, 1, '', 'raw-key-on-disk')
     ON CONFLICT (tenant_id, version)
     DO UPDATE SET wrapped_key = '', algorithm = 'raw-key-on-disk';
     ```
     Both `wrapped_key` and `algorithm` are explicitly set — **not** just
     `wrapped_key`. This is required for the profile-switch-back case
     (production → local-dev): if a prior `rewrap` left `algorithm =
     'aes256gcm-argon2id'`, an UPDATE that sets only `wrapped_key = ''` would
     leave the wrong algorithm sentinel and cause the unwrap branch to attempt
     decryption of an empty ciphertext. Zero and delete the old file.
   - *`production` profile:* If `.reborn-local-dev-secrets-master-key` exists
     and `brassclaw_secrets_master` has no row for this tenant, **do not
     auto-migrate**. Print:
     `"Run 'brassclaw secrets rewrap' before starting the service in production."`
     and exit with a non-zero code. The operator must run `brassclaw secrets rewrap`
     interactively once, then restart.
7. **Migrate libSQL database:** If `reborn-local-dev.db` exists, open it
   with the libSQL crate (gated behind a `migrate-from-libsql` feature — see
   §9 for why this feature must be **enabled by default** in the migration
   release), read every table, and write the rows into the corresponding
   Postgres tables using upsert-or-ignore semantics. Rename the file to
   `reborn-local-dev.db.migrated` when done. Record that migration occurred.

   **Tenant/user/project synthesis for libSQL rows that lack these columns
   (H1):** Several libSQL tables have no `tenant_id` (or `user_id` /
   `project_id`) column because the old schema was single-tenant. The
   migration must synthesise these values for every migrated row:

   | libSQL table | Missing columns | Synthesised from |
   |---|---|---|
   | `safety_config` | `tenant_id` | `boot_tenant` (see below) |
   | `settings` (token settings) | `tenant_id`, `user_id` | `boot_tenant`, `boot_user` |
   | `memory_docs` | `tenant_id`, `user_id`, `project_id` | `boot_tenant`, `boot_user`, `"default"` |
   | `root_filesystem_entries` | `tenant_id` | `boot_tenant` |
   | `capability_permissions` | `tenant_id` | `boot_tenant` |
   | `hooks_predicate_invocations/values` | *(none — no tenant column in source)* | n/a; appended verbatim |

   `boot_tenant` = value of `brassclaw_config.identity.tenant` if already
   written by step 3 (migrated from `config.toml`), otherwise the literal
   string `"default"`. `boot_user` = `brassclaw_config.identity.owner` if
   written by step 3, otherwise `"admin"`. These are the same defaults the
   first-run wizard uses (§6.2 Step 3/4). The synthesised values are written
   into the PG rows at insert time; the libSQL rows are not modified.
8. **Set `boot.initialized = true`** in `brassclaw_config` — **only if at
   least one migration step (3–7) actually found and processed a source
   artifact.** For a completely fresh install (no pre-existing files), leave
   `boot.initialized` absent so the first-run wizard runs (§6.1). The wizard
   itself sets `boot.initialized = true` at the end.

All steps are idempotent (upsert / rename). Re-running the migration after
a crash is safe.

**migration-dry-run profile:** Steps 3–8 run in read-only simulation mode —
no DB writes, no file renames. Results are printed and the process exits.

---

## 9. Feature Flag Cleanup

After the migration, these Cargo features are **removed**:

- `libsql` on every crate
- `postgres` on every crate (Postgres is now the only backend, not a feature)

The `embedded-postgres` feature on `brassclaw_reborn_composition` remains so
callers that supply their own `BRASSCLAW_PG_URL` do not pay the binary-size
cost of bundling `postgresql_embedded`.

### 9.1 `migrate-from-libsql` feature lifecycle

The `migrate-from-libsql` feature (workspace `Cargo.toml`) gates the libSQL
read path used in §8.1 step 7. It must be **on by default** in the single
upgrade release so that no user with an existing `reborn-local-dev.db` silently
loses data. Concretely: the release `Cargo.toml` must have:

```toml
[features]
# Upgrade release: migrate-from-libsql is on by default.
# libsql is pulled transitively by migrate-from-libsql = ["dep:libsql"].
# The postgres/html-to-markdown/tui features remain in default as before.
default = ["migrate-from-libsql", "postgres", "html-to-markdown", "tui"]
migrate-from-libsql = ["dep:libsql"]
# postgres, html-to-markdown, tui defined as before (unchanged)
```

The feature and all code behind it are removed in the **following** release
after one full upgrade cycle. The integration test gating on this feature
(`seed_libsql_then_migrate_asserts_all_rows_in_pg`) must be green in CI
before the migration release ships.

### 9.2 `replay` and `import` features

The root `Cargo.toml` currently defines:
```toml
replay = ["libsql"]         # memory-substrate regression tests
import = ["dep:json5", "libsql"]  # OpenClaw import
```

Both depend on `libsql`, which is being removed. Decision:

- **`replay`:** Rebase the replay-gate test harness onto the embedded Postgres
  test rig (Phase 6 work item). The `replay-gate.yml` CI job is updated to
  spin up embedded PG instead of libSQL. The `replay` feature becomes
  `replay = ["postgres"]` (or is removed if the test rig is always-on).
- **`import`:** Port the OpenClaw import path off libSQL. Until ported, the
  `import` feature is **removed** from the default set and marked deprecated
  in `CHANGELOG.md`. A follow-up issue is filed.

Both must be resolved explicitly in Phase 6; the plan does not leave them
referencing a non-existent `libsql` feature.

---

## 10. Implementation Phases

> **Phase ordering note:** "Phase 2 — Config migration" is the first *content*
> phase but not the first *execution* phase. Phase 0 (embed PG) and Phase 1
> (schema runner) are prerequisites; implementation starts there.

### Phase 0 — Embedded Postgres crate (no existing code touched)
- [ ] Create `crates/brassclaw_embedded_postgres/`
- [ ] `postgresql_embedded` integration, pinned PG 16
- [ ] `checksums.rs`: compiled-in SHA-256 list; verify after download; suppress `POSTGRESQL_VERSION` env override
- [ ] `initdb`, `pg_ctl` lifecycle, `health.rs` retry loop
- [ ] Orphaned-server detection: check `postmaster.pid` PID liveness on startup
- [ ] Explicit `shutdown()` method; `Drop` is best-effort fallback only
- [ ] Log rotation config in `postgresql.conf` (§2.2)
- [ ] Unit tests (mock pg_ctl, verify postgresql.conf tuning)

### Phase 1 — Schema and migration runner
- [ ] Create `crates/brassclaw_pg/`
- [ ] Write all `V000__` … `V018__` SQL migration files (all `IF NOT EXISTS`; all `CREATE TRIGGER`)
- [ ] Wire `refinery` runner with history-reconciliation bootstrap (§3)
- [ ] `PgPool` builder (from URL or from `ManagedPostgres` handle)
- [ ] Test: fresh DB gets all tables; re-run is idempotent
- [ ] Test: pre-existing hooks/settings tables don't cause refinery to fail

### Phase 2 — Config migration (start content work here, after Phase 0+1)
- [ ] `brassclaw_reborn_composition::db_config` module (`load_config_snapshot`, `save_config_key`)
- [ ] Confirm `db_config.rs` is NOT added to `brassclaw_reborn_config` — boundary crate stays pure (§5.4)
- [ ] Replace **runtime serve-path** `RebornConfigFile::load` callers with `db_config::load_config_snapshot`; retain `load()` behind `migrate-from-libsql` for §8.1 step 3 + §4.4 rewrap step 2 (§5.4 — removal deferred to next release)
- [ ] First-run wizard (`brassclaw config init --interactive`)
- [ ] `brassclaw config` CRUD subcommands (get/set/unset/list/show-all/export/import)
- [ ] `ProviderRepo` → DB-backed
- [ ] `sempai_provider.json` → `brassclaw_config` rows
- [ ] Test: round-trip all `RebornConfigFile` sections through DB
- [ ] Test: `save_config_key(…, ConfigWriteContext::AgentSession)` returns `EnvKeyWriteForbidden` for keys ending in `_env` (§5.5 / §1c write-gate)
- [ ] Test: `save_config_key(…, ConfigWriteContext::AgentSession)` succeeds for non-`*_env` keys (gate is scoped to `_env` suffix only — not a blanket `AgentSession` write ban)
- [ ] Test: `save_config_key(…, ConfigWriteContext::Operator)` succeeds for `*_env` keys (operator path not blocked)
- [ ] Test: boolean/integer/decimal config values survive DB round-trip (serialization contract §4.2)
- [ ] Remove `toml_edit`, `fs4`, `tempfile` from `brassclaw_reborn_config`; keep `toml`

### Phase 3 — Secrets migration
- [ ] `PgSecretStore` and `PgCredentialBroker`
- [ ] `brassclaw_secrets_master` with `key_version` (§4.4 schema)
- [ ] local-dev: 0600 raw key file at `$REBORN_HOME/.secrets-master-key`
- [ ] `brassclaw secrets rewrap --strategy passphrase|passphrase-file=<path>|keychain [--tenant <id>]` (§4.4)
- [ ] `rewrap` tenant resolution: `--tenant` flag → `config.toml identity.tenant` → `brassclaw_config` DB → `"default"` (§4.4 R6-MH1)
- [ ] `rewrap --old-passphrase-file=<path>` flag for interactive passphrase-change in shell (§4.4 R6-L1)
- [ ] `rewrap` key-source rule: check old filename `.reborn-local-dev-secrets-master-key` first, then `.secrets-master-key`; fail-closed if neither found but encrypted rows exist (§4.4)
- [ ] `rewrap` passphrase-change path: unwrap existing wrapped key; read old passphrase from `--old-passphrase-file` → `BRASSCLAW_SECRETS_PASSPHRASE_FILE` → `$CREDENTIALS_DIRECTORY` (§4.4)
- [ ] Per-boot unwrap: read `BRASSCLAW_SECRETS_PASSPHRASE_FILE` at serve startup (production only)
- [ ] Fail-closed in production profile if master key absent AND no raw key file AND no passphrase file
- [ ] Abstract secret-value reads to check `$CREDENTIALS_DIRECTORY` (systemd LoadCredential) first, env second (§7)
- [ ] Migration from `.reborn-local-dev-secrets-master-key` (§8.1 step 6)

### Phase 4 — Runtime store migrations (one crate at a time)
- [ ] `PgRunStateStore` (in `brassclaw_run_state`)
- [ ] `PgApprovalRequestStore` (in `brassclaw_approvals`)
- [ ] `PgTurnStateStore` + `PgLoopCheckpointStore` (in `brassclaw_turns`)
- [ ] `PgCheckpointStateStore` (in `brassclaw_loop_support`)
- [ ] `PgSessionThreadService` (in `brassclaw_threads`)
- [ ] `PgCapabilityLeaseStore` (in `brassclaw_authorization`)
- [ ] `PgResourceGovernorStore` (in `brassclaw_resources`)
- [ ] `PgProcessStore` + `PgProcessResultStore` (in `brassclaw_processes`)
- [ ] `PgExtensionInstallationStore` (in `brassclaw_extensions`) + extension manifests table
- [ ] `PgDurableEventLog` + `PgDurableAuditLog` (in `brassclaw_events`)
- [ ] `PgTokenSettingsStore` (in `brassclaw_reborn_composition`)
- [ ] `PgSafetyConfigStore` + `PgCapabilityPermissions` (in `brassclaw_product_workflow`)
- [ ] `PgMemoryDocStore` with generated-column GIN FTS (in `brassclaw_reborn_composition`)
- [ ] Background retention sweep task in `brassclaw serve` only (§4.21); add `brassclaw maintenance prune-old-data` CLI command
- [ ] `PgResourceGovernorStore`: implement CAS via `version` column conditional UPDATE; return `BudgetConflict` on 0-rows-affected; integration test for concurrent increments (§4.12)
- [ ] Verify `brassclaw_root_filesystem` queries in `PostgresRootFilesystem` always scope to `tenant_id` (§4.19)

### Phase 5 — Hooks and auth
- [ ] Rename `brassclaw_hooks_postgres` → `brassclaw_hooks_pg`, make deps mandatory
- [ ] Delete `brassclaw_hooks_libsql` and `brassclaw_hooks_parity`
- [ ] `PgAuthProductServices`
- [ ] Wire `brassclaw_reborn_composition::factory` to single Postgres path
- [ ] Pool drop before `managed_pg.shutdown().await`

### Phase 6 — libSQL removal
- [ ] Rebase `replay` feature onto embedded Postgres test rig (§9.2)
- [ ] Deprecate/remove `import` feature; file follow-up issue for OpenClaw port (§9.2)
- [ ] Delete all `#[cfg(feature = "libsql")]` and `#[cfg(not(feature = "libsql"))]` blocks
- [ ] Remove `libsql` from all `Cargo.toml` files (except `migrate-from-libsql` for upgrade release)
- [ ] Remove `libsql` feature gates from workspace `Cargo.toml`
- [ ] Delete `brassclaw_hooks_libsql` crate directory
- [ ] Update `brassclaw_architecture` boundary tests

### Phase 7 — libSQL → Postgres data migration at boot

> **Phase ordering constraint (L1):** Phase 7 depends on Phase 6 for the
> `migrate-from-libsql` feature flag. However, Phase 6 (libSQL removal) must
> NOT be merged before Phase 7's migration code is complete and green in CI.
> The correct sequence is:
> 1. Implement Phase 7 (migration code + integration test) gated behind
>    `migrate-from-libsql` feature.
> 2. Merge Phase 6 **only after** Phase 7 is complete. Phase 6 removes the
>    unconditional libSQL dep; Phase 7's `migrate-from-libsql` transitively
>    keeps the dep available for the upgrade release only.
> 3. In the release after the upgrade release, remove Phase 7 code + feature.
> If Phase 6 is merged before Phase 7 is complete, the `migrate-from-libsql`
> feature will reference a non-existent crate and the upgrade migration code
> won't compile. The CI gate on `seed_libsql_then_migrate_asserts_all_rows_in_pg`
> enforces this — it must be green before Phase 6 merges.

- [ ] `brassclaw_reborn_composition::migration` module
- [ ] Steps 3–7 from §8.1 (including profile-aware secrets step)
- [ ] `migrate-from-libsql` is **default-on** in the upgrade release (§9.1)
- [ ] Integration test: seed a libSQL DB, run migration, verify all rows land in PG
- [ ] **Integration test (upgrade-flow decryption, gate migration release):** seed a
      libSQL DB with an encrypted secret (OAuth token encrypted under the old raw master
      key); run `rewrap` (reads old key, wraps it); run `serve` (§8.1 migrates rows);
      assert the migrated secret decrypts correctly with the wrapped key. Must be green
      before migration release ships.
- [ ] Test: `boot.initialized` is NOT set on a completely fresh install (wizard runs)
- [ ] Test: `boot.initialized` IS set when any source artifact was found
- [ ] **Integration test (non-default tenant upgrade, gate migration release — R6-MH1):**
      seed a config.toml with `identity.tenant = "mycorp"` (not `"default"`); run
      `rewrap --tenant mycorp --strategy passphrase-file=...`; assert the
      `brassclaw_secrets_master` row has `tenant_id = "mycorp"`; run `serve`; assert
      §8.1 step 6 finds the row (no non-zero exit); assert a migrated encrypted
      `root_filesystem_entries` row decrypts correctly under the wrapped key.
      Must be green before migration release ships.
- [ ] Phase 3: implement `rewrap --tenant <id>` flag and 4-step tenant-resolution
      logic (`--tenant` → `config.toml` → `brassclaw_config` DB → `"default"`)
- [ ] Phase 9: document `--old-passphrase-file=<path>` in passphrase-rotation runbook
- [ ] Rename `.migrated` files; remove `migrate-from-libsql` feature in next release
- [ ] Remove `RebornConfigFile::load()` in this same next release (gated removal — §5.4)

### Phase 8 — File-based config removal
- [ ] Remove `config_file_path()`, `providers_file_path()`, `sempai_provider_file_path()`
      from `RebornHome`
- [ ] Remove `config.toml.lock`, `providers.json.lock` discipline from `ProviderRepo`
      and `DefaultLlmSlotUpdateSession`
- [ ] Update `brassclaw_reborn_cli` `config init` → wizard

### Phase 9 — Systemd unit and documentation
- [ ] Write `brassclaw.service` systemd unit template (§7.1/§7.2/§7.3 including hardening directives)
- [ ] Update `AGENTS.md` Database Rules section (retire dual-backend mandate — §0a)
- [ ] **Purge stale v1 `src/` sections from `CLAUDE.md`** — `src/db/`, `src/channels/`,
      `src/agent/`, `src/workspace/`, `src/sandbox/`, `src/registry/`, `src/tunnel/`
      all describe code removed in Phase 6 that must not mislead new contributors
- [ ] Update `CLAUDE.md` env var table: document the two-tier model (bootstrap tier 6 fixed vars + operator-trusted env tier data-driven); remove retired `DATABASE_BACKEND`/`LIBSQL_*`/`LLM_*`/`GOOGLE_CLIENT_ID` vars
- [ ] Write operator guide covering: prerequisites (§7.0), fresh-install sequence (§7.1),
      upgrade sequence (§7.2), `master.key` ownership requirements, `master.key` DR backup
      mandate (§11), CLI-only users and `brassclaw maintenance prune-old-data` (§4.21),
      `rewrap` vs `rotate` distinction (§4.4)
- [ ] Update all per-crate `CLAUDE.md`/`AGENTS.md` spec files
- [ ] Update `CHANGELOG.md`
- [ ] Add architecture test: no `std::fs::read_to_string` / `File::open` in any
      non-migration production path

### Phase 10 — Integration tests and E2E
- [ ] Integration test: full boot cycle from scratch (embedded PG starts, wizard runs,
      agent serves a turn, graceful shutdown stops PG — including explicit `shutdown()`)
- [ ] Integration test: restart resumes existing state from Postgres
- [ ] Integration test: `BRASSCLAW_PG_URL` override (no embedded PG spawned)
- [ ] Integration test: SIGKILL → restart → orphaned-server detection and reuse
- [ ] E2E: provider add/edit/delete via WebUI persists across restart
- [ ] **Hardened-unit integration test (gate migration release):** embedded PG starts
      and serves a query under the §7 hardening directives
      (`MemoryDenyWriteExecute=yes`, `SystemCallFilter=@system-service`,
      `RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX`). Validates that
      `jit=off` in `postgresql.conf` is sufficient to prevent the MDWE JIT crash.
      This test must be green before the migration release ships.
- [ ] Integration test: `brassclaw config get <key>` against a running `brassclaw serve`
      does not stop embedded PG (conditional-shutdown rule, §6.4 step 4).

---

## 11. Risks and Mitigations

| Risk | Mitigation |
|---|---|
| `postgresql_embedded` download fails in offline/air-gapped env | Detect no-network at startup; print: "set `BRASSCLAW_PG_URL` to an external Postgres". Ship a `--download-pg` CLI helper that pre-caches the binary. |
| Supply-chain: zonky binary compromise | Compiled-in SHA-256 per version (§2.2); `POSTGRESQL_VERSION` env override suppressed; production deployments use `BRASSCLAW_PG_URL` to an operator-managed PG. |
| Large binary download on first run (~40 MB) | Cached in `$REBORN_HOME/postgres/bin/` after the first download. Progress bar shown. |
| Embedded PG orphaned after SIGKILL | On next start: check `postmaster.pid` liveness (kill -0). If alive, reuse the server. If PID is dead, remove stale PID file and restart (§2.2). |
| Port 5434 already in use | TCP-probe at startup; fail: "port 5434 in use — set `BRASSCLAW_PG_URL` or `BRASSCLAW_EMBEDDED_PG_PORT`". |
| PG log dir fills disk | Log rotation configured in `postgresql.conf`: 50 MB cap, daily rotation (§2.2). |
| Pool still open when `Drop` tries to stop PG | `managed_pg.shutdown()` called explicitly after pool close; `Drop` is last-resort only (§2.2, §5.5). |
| Data migration from libSQL loses rows | `migrate-from-libsql` default-on in upgrade release (§9.1); upsert-or-ignore; original `.db` renamed, not deleted; fail-loud if feature is off (§8.1). |
| `boot.initialized` set on fresh install, wizard skipped | Step 8 is conditional: only set if a source artifact was found (§8.1). |
| `config.toml` edited by operators who don't know about the change | `brassclaw config import < config.toml` import command; doc the new workflow. |
| Architecture tests break during libSQL removal | Remove in Phase 6 explicitly; add Postgres-boundary replacements. |
| `brassclaw_reborn_config` boundary test broken | `db_config.rs` lives in `brassclaw_reborn_composition` (§5.4/§5.5); boundary test unchanged. |
| Production headless boot: no passphrase source | `BRASSCLAW_SECRETS_PASSPHRASE_FILE` (bootstrap tier) required for unattended boot. `keychain` strategy requires a desktop session. See §4.4 full strategy table and §7 first-boot sequences. |
| Interactive wizard hangs under systemd | `brassclaw serve` checks `isatty(stdin)` before launching wizard; fails with clear error if not a TTY (§6.1). Use §7.2 upgrade or §7.1 fresh-install sequence instead. |
| Dual-backend AGENTS.md rule violated | Explicitly documented in §0a; rule rewrite is a first-class Phase 9 deliverable requiring sign-off. |
| PG JIT crashes under `MemoryDenyWriteExecute=yes` | `jit=off` set in `postgresql.conf` (§2.2); hardened-unit integration test in Phase 10 gates the migration release. |
| `config init` on upgrade clobbers migrated config | §7 documents fresh-install (§7.1) vs upgrade (§7.2) sequences explicitly. Upgrade sequence omits `config init`. |
| `rewrap` on upgrade fails: `brassclaw_secrets_master` does not exist | `rewrap` starts embedded PG and runs schema migrations (§6.4) before writing; table exists before `serve` ever starts. |
| `rewrap` generates new key on upgrade → migrated secrets undecryptable | Key-source rule (§4.4): `rewrap` checks `.reborn-local-dev-secrets-master-key` first, then `.secrets-master-key`; fails closed if neither found but encrypted rows exist. Upgrade-flow decryption integration test in Phase 7 gates release. |
| Concurrent CLI (`config get`) crashes `serve` via conditional PG shutdown | §6.4 step 4: CLI shuts down PG only if it started it; if a live postmaster is detected (from `serve`), leave it running. Phase 10 integration test gates release. |
| Passphrase change (`rewrap`) fails: old passphrase unavailable in interactive shell | §4.4: use `--old-passphrase-file=<path>` for shell invocation; `BRASSCLAW_SECRETS_PASSPHRASE_FILE` / `$CREDENTIALS_DIRECTORY` are the systemd-injected fallbacks. Document in passphrase-rotation runbook (Phase 9). |
| `rewrap` tenant ≠ `boot_tenant` → boot failure + orphaned ciphertext (R6-MH1) | `rewrap` resolves `tenant_id` via: `--tenant` flag → `config.toml` `identity.tenant` → `brassclaw_config` DB → `"default"`. §7.1/§7.2 runbook commands pass `--tenant` explicitly. Phase 7 non-default-tenant upgrade integration test gates release. Since `rewrap` zeros the raw key file on success, a tenant mismatch combined with a missing `brassclaw_secrets_master` row causes a non-recoverable boot failure — never suppress the fail-closed exit. |
| master.key is root-owned → EACCES at per-boot unwrap | All setup steps that write `master.key` must run as the service user (`sudo -u brassclaw`) — see §7.0/§7.1/§7.2. |
| master.key lost in disaster recovery | The passphrase file (`master.key`) **must be in the operator's DR backup set**, separately from the Postgres data directory. Loss of `master.key` without a backup means all `brassclaw_secrets` rows (OAuth tokens, credential-broker creds) are permanently unrecoverable. Document in operator guide (Phase 9). |
| Passphrase-file wrap provides no extra protection on same-host embedded PG | Wrapping earns its threat-model value when PG is remote (`BRASSCLAW_PG_URL`): a DB-only breach (backup leak, remote PG compromise) cannot decrypt secrets without `master.key` on the app host. On single-host embedded PG, the security model is equivalent to a raw 0600 key file. Operators who require stronger isolation should use `BRASSCLAW_PG_URL`. |

---

## 12. Files Modified Summary (anticipated)

| File / path | Change |
|---|---|
| `Cargo.toml` (workspace) | Add `brassclaw_embedded_postgres`, `brassclaw_pg`; add `migrate-from-libsql` (default-on for upgrade release); remove `brassclaw_hooks_libsql`; rebase `replay`, remove `import` (§9.2) |
| `crates/brassclaw_embedded_postgres/` | **New crate** |
| `crates/brassclaw_pg/` | **New crate** (migrations + pool) |
| `crates/brassclaw_hooks_pg/` | Rename from `brassclaw_hooks_postgres`; remove optional feature gate |
| `crates/brassclaw_hooks_libsql/` | **Deleted** |
| `crates/brassclaw_hooks_parity/` | **Deleted** |
| `crates/brassclaw_reborn_config/src/config_file.rs` | Remove `write()`; retain `load()` behind `migrate-from-libsql` (needed by §8.1 step 3 + §4.4 rewrap step 2 — removed in the same next release that drops `migrate-from-libsql`, per §5.4) |
| `crates/brassclaw_reborn_config/src/home.rs` | Remove `config_file_path()`, `providers_file_path()`, `sempai_provider_file_path()` |
| `crates/brassclaw_reborn_config/Cargo.toml` | Remove `toml_edit`, `fs4`, `tempfile`. **Keep `toml`**. Do NOT add `deadpool-postgres`. |
| `crates/brassclaw_reborn_composition/src/db_config.rs` | **New file** — `load_config_snapshot`, `save_config_key(…, caller: ConfigWriteContext)`, `ConfigWriteContext` enum; `*_env` key write-gate (§5.5, §1c) |
| `crates/brassclaw_reborn_composition/src/factory.rs` | Remove libSQL/file branches; single Postgres path; explicit `shutdown()` before pool drop |
| `crates/brassclaw_reborn_composition/src/provider_repo.rs` | DB-backed rewrite |
| `crates/brassclaw_reborn_composition/src/llm_config_service.rs` | Use `db_config::load_config_snapshot`; remove file paths |
| `crates/brassclaw_reborn_composition/Cargo.toml` | Remove `libsql`; add `brassclaw_embedded_postgres` |
| `crates/brassclaw_reborn_cli/src/commands/` | Add first-run wizard; rewrite `config init` |
| `crates/brassclaw_filesystem/` | Remove libSQL backend; add Postgres fallback backend |
| `crates/brassclaw_run_state/` | Add `PgRunStateStore`; delegate approvals to `brassclaw_approvals` |
| `crates/brassclaw_approvals/` | Add `PgApprovalRequestStore` (dedicated crate — see §5.3 ⬡) |
| `crates/brassclaw_turns/` | Add `PgTurnStateStore`, `PgLoopCheckpointStore`; remove libSQL impls |
| `crates/brassclaw_loop_support/` | Add `PgCheckpointStateStore`; remove libSQL impl (see §5.3 ⬡) |
| `crates/brassclaw_threads/` | Add `PgSessionThreadService`; remove libSQL impl |
| `crates/brassclaw_authorization/` | Add `PgCapabilityLeaseStore`; remove libSQL impl |
| `crates/brassclaw_resources/` | Add `PgResourceGovernorStore`; remove libSQL impl |
| `crates/brassclaw_processes/` | Add `PgProcessStore`, `PgProcessResultStore` (with `tenant_id`); remove libSQL impl |
| `crates/brassclaw_extensions/` | Add `PgExtensionInstallationStore` (moved from composition — see §5.3 ⬡) |
| `crates/brassclaw_events/` | Add `PgDurableEventLog`, `PgDurableAuditLog` |
| `crates/brassclaw_secrets/` | Add `PgSecretStore`, `PgCredentialBroker`; add `key_version` support |
| `crates/brassclaw_product_workflow/` | Replace `SqliteSafetyConfigStore` with `PgSafetyConfigStore` (natural-key UNIQUE) |
| `crates/brassclaw_architecture/` | Update boundary tests |
| `CLAUDE.md` | Update database section; purge stale v1 `src/` docs; update env var table |
| `AGENTS.md` | Retire dual-backend rule (§0a) |
| `CHANGELOG.md` | Entry for this migration |
