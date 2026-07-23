# The Great Review — BrassClaw Codebase

**Scope:** All files modified in the current working tree plus newly-added migrations and the `sweepfix.py` script.  
**Toolchain:** `rustc 1.95.0` / `stable-aarch64-apple-darwin`

---

## Findings & Remediation Plan

Each issue is numbered, triaged by severity, and links to the affected file/line. Every step must be completed in order because some fixes depend on earlier ones.

---

### Issue 1 — CRITICAL: Class-code mismatch in `COMPONENT_TABLES` (wrong labels in assembled prompts)

**File:** [`crates/brassclaw_reborn_composition/src/interceptor_config_service.rs`](crates/brassclaw_reborn_composition/src/interceptor_config_service.rs) lines 42–57  
**Root cause:**  
`COMPONENT_TABLES` records a `(table_name, class_code)` pair per table. That class code is **not** used to filter SQL rows — the query selects any `validation_status = 'validated'` row from the table. The class code is used only as a label in the assembled Sempai base prompt (`## {class_code}:{prompt_uid}  {label}  "{name}"`). The values currently stored are wrong compared to the `class_code` column values enforced by the DB check constraints:

| Table | DB `class_code` | Code value | Impact |
|---|---|---|---|
| `reborn_specs` | 12 | 11 | Prompt header says `11:N Spec` |
| `reborn_summaries` | 15 | 12 | Prompt header says `12:N Summary` |
| `reborn_lessons` | 18 | 13 | Prompt header says `13:N Lesson` |
| `reborn_issues` | 19 | 14 | Prompt header says `14:N Issue` |
| `reborn_notes` | 20 | 15 | Prompt header says `15:N Note` |
| `reborn_tool_skills` | 13 | 22 | Prompt header says `22:N ToolSkill` |
| `reborn_plans` | 14 | 8 | Prompt header says `8:N Plan` |
| `reborn_extensions_unified` | (check V032) | 9 | may be correct |
| `reborn_orchestrators` | (check V029) | 10 | may be correct |

**Fix:** Correct the class codes in `COMPONENT_TABLES` to match the DB DDL. Also correct the `class_label()` match arms accordingly so the human-readable section headers remain accurate.

**Action:** Edit `interceptor_config_service.rs` lines 42–77.

---

### Issue 2 — HIGH: `tracing::info!` in background task / `tracing::warn!` in startup code

**File:** [`crates/brassclaw_reborn_cli/src/commands/serve.rs`](crates/brassclaw_reborn_cli/src/commands/serve.rs)  
**Lines:**  
- 235–239: `tracing::warn!` — Notion DCR OAuth not configured  
- 313–317: `tracing::warn!` — binding on non-loopback interface  
- 472–475: `tracing::info!` — ctrl-c received inside a `tokio::spawn` background task  
- 510: `tracing::warn!` — embedded Postgres shutdown failed  

**Rule (AGENTS.md §67 / CLAUDE.md §169):** `info!` and `warn!` output appears in the REPL and corrupts the terminal UI. Background tasks must never use `info!`. Use `debug!` for internal diagnostics.

**Analysis:**  
- Lines 235–239 and 313–317 are startup-time operator hints, not background-task work. They currently corrupt the WebChat UI on start.  
- Line 472–475 is inside a `tokio::spawn` background task — explicit violation.  
- Line 510 is a shutdown warning — safe to keep as `warn!` because it fires only after the WebUI has already been torn down (post-shutdown path, not mid-operation). This one can remain `warn!`.

**Fix:**  
- Lines 235–239 and 313–317: Change to `eprintln!` (startup operator messages, same pattern already used at line 304).  
- Line 472–475: Change `tracing::info!` → `tracing::debug!`.  
- Line 510: Leave as `warn!` (correct — shutdown path, UI is already gone).

**Action:** Edit `serve.rs`.

---

### Issue 3 — HIGH: Silent `unwrap_or` masking DB column read failures

**File:** [`crates/brassclaw_reborn_composition/src/interceptor_config_service.rs`](crates/brassclaw_reborn_composition/src/interceptor_config_service.rs) lines 251–253  
```rust
let prompt_uid: i64 = row.try_get("prompt_uid").unwrap_or(0);
let name: String     = row.try_get("name").unwrap_or_default();
let content: String  = row.try_get("content").unwrap_or_default();
```
If the column is missing (e.g., after a schema change), the row is silently assembled with `(0, "", "")` and included in the Sempai base prompt as an empty/garbage section — rather than the table being skipped with a debug log.

**Fix:** Replace `unwrap_or` with explicit error logging via `tracing::debug!` and `continue` to skip the bad row. This keeps the resilience but makes failures observable.

**Action:** Edit `interceptor_config_service.rs` lines 250–255.

---

### Issue 4 — MEDIUM: Missing `#[deprecated]` attribute on deprecated function

**File:** [`crates/brassclaw_reborn_composition/src/input.rs`](crates/brassclaw_reborn_composition/src/input.rs) line 329  
The function `postgres_with_resolved_secret_master_key` has a doc-comment saying "Deprecated: prefer `postgres_with_reborn_home`" but lacks the Rust `#[deprecated]` attribute. Callers will never get a compiler warning.

**Fix:** Add `#[deprecated(note = "prefer `postgres_with_reborn_home`")]` above the function.

**Action:** Edit `input.rs` line ~332.

---

### Issue 5 — MEDIUM: `scripts/sweepfix.py` embeds unsanitized branch names in commit messages

**File:** [`scripts/sweepfix.py`](scripts/sweepfix.py) line 234  
```python
result = run(["git", "merge", "--no-ff", br, "-m", f"chore: merge {br} → main"], ...)
```
`br` is a branch name read from the local git repository. While `subprocess.run` with a list prevents shell injection, a malicious branch name containing newlines or special characters would produce a malformed commit message that could confuse downstream tooling (CI log parsers, CHANGELOG generators, signed-commit verifiers).

**Fix:** Sanitize `br` before embedding in the commit message: replace any character not in `[a-zA-Z0-9/_.-]` with `_`.

**Action:** Add sanitization helper in `sweepfix.py` and apply it at line 234.

---

### Issue 6 — LOW: V046 migration comment inaccuracy ("seven former-DocType" vs actual eight)

**File:** [`crates/brassclaw_pg/migrations/V046__reborn_component_tables_solution_columns.sql`](crates/brassclaw_pg/migrations/V046__reborn_component_tables_solution_columns.sql) line 3  
The comment says "seven former-DocType component tables" but the migration body operates on **eight** tables: `reborn_specs`, `reborn_tool_skills`, `reborn_plans`, `reborn_summaries`, `reborn_docus`, `reborn_lessons`, `reborn_issues`, `reborn_notes`.

**Fix:** Change "seven" to "eight" in the comment.

**Action:** Edit V046 migration file line 3.

---

### Issue 7 — LOW: `webui_auth.rs` `.expect()` without a verifiable invariant comment

**File:** [`crates/brassclaw_reborn_cli/src/commands/webui_auth.rs`](crates/brassclaw_reborn_cli/src/commands/webui_auth.rs) line 119  
```rust
.expect("non-empty providers always produce login wiring")
```
The safety comment is on the same line and references `sso_startup_config_from_env`. This is acceptable (the invariant holds based on the call site), but the comment should reference the specific function that guarantees it so it's auditable.

**Fix:** Expand the safety comment to be explicit about which guarantee is being relied on.

**Action:** Edit `webui_auth.rs` line 119 (comment only).

---

## Execution Order

- [x] **Step 1** — Fix class-code mismatch in `COMPONENT_TABLES` and `class_label()` in `interceptor_config_service.rs` *(resolved in commit 64adcadf)*
- [x] **Step 2** — Replace silent `unwrap_or` with error-logged skip in `interceptor_config_service.rs` *(resolved in commit 64adcadf)*
- [x] **Step 3** — Fix logging violations in `serve.rs` (`warn!` → `eprintln!`, `info!` → `debug!`) *(resolved in commit 64adcadf)*
- [x] **Step 4** — Add `#[deprecated]` attribute to `postgres_with_resolved_secret_master_key` in `input.rs` *(resolved in commit 64adcadf)*
- [x] **Step 5** — Sanitize branch names in `sweepfix.py` commit message *(resolved in commit 64adcadf)*
- [x] **Step 6** — Fix "seven" → "eight" in V046 migration comment *(resolved in commit 64adcadf)*
- [x] **Step 7** — Improve safety comment on `.expect()` in `webui_auth.rs` *(resolved in commit 64adcadf)*
- [x] **Step 8** — Run `cargo clippy --all --benches --tests --examples --all-features -- -D warnings` — zero warnings *(verified)*
- [x] **Step 9** — Run `cargo test -p brassclaw_reborn_composition -p brassclaw_reborn_cli` — all tests pass *(verified)*

---

## Round 2 Findings

### Issue 8 — MEDIUM: `tracing::warn!` in `pool.rs` and `Drop` impl corrupts terminal UI

**Files:**
- [`crates/brassclaw_pg/src/pool.rs`](crates/brassclaw_pg/src/pool.rs) lines 61, 65, 90–94
- [`crates/brassclaw_embedded_postgres/src/lib.rs`](crates/brassclaw_embedded_postgres/src/lib.rs) line 152

**Rule:** AGENTS.md §67 — `warn!` corrupts the terminal UI; use `eprintln!` for startup-time operator notices, `debug!` for internal diagnostics.

**Analysis:**
- `pool.rs` lines 61 and 65 emit `warn!` for individual TLS cert load failures — these fire during pool construction (startup path). Change to `debug!`.
- `pool.rs` lines 90–94 emit `warn!` when a remote PG URL lacks `sslmode=` — this is an intentional operator-visible security reminder. The doc-comment calls it "non-suppressible". Change to `eprintln!` (same pattern as `serve.rs`).
- `lib.rs` line 152: `warn!` in `Drop` — fired when the struct is dropped without calling `shutdown()`. This can occur mid-operation if ManagedPostgres is accidentally dropped. Change to `eprintln!` so it is always visible without polluting tracing.

**Fix:** `pool.rs` cert errors → `debug!`; SSL warning → `eprintln!`; `lib.rs` Drop warning → `eprintln!`.

---

### Issue 9 — MEDIUM: Silent DB error in `load_config()` returns empty config

**File:** [`crates/brassclaw_reborn_composition/src/interceptor_config_service.rs`](crates/brassclaw_reborn_composition/src/interceptor_config_service.rs) line 130

**Code:**
```rust
list_config_keys(&self.pool, &self.tenant_id)
    .await
    .unwrap_or_default()  // DB error returns empty HashMap silently
```

**Impact:** If the database is unavailable, the Sempai interceptor silently appears unconfigured (no base prompt, no persona) rather than surfacing an error. The caller (`snapshot()`, `update()`, `do_reassemble()`) never learns the DB was down.

**Fix:** Add a `tracing::debug!` log before returning the empty fallback so operators and tests can diagnose the silent fallback. The `unwrap_or_default` itself is acceptable for resilience (the interceptor should still function without its optional config), but the failure must be observable.

---

### Issue 10 — MEDIUM: Stale PID file removal failure silently ignored

**File:** [`crates/brassclaw_embedded_postgres/src/lib.rs`](crates/brassclaw_embedded_postgres/src/lib.rs) line 87

**Code:**
```rust
let _ = tokio::fs::remove_file(&pid_file).await;
```

**Impact:** If removal fails (permissions, read-only filesystem), startup continues silently but `pg_ctl start` will subsequently fail with a confusing error about a stale PID file rather than a clear "could not remove stale postmaster.pid" message.

**Fix:** Log the error at `debug!` level when removal fails (not `warn!` — avoids UI corruption per convention).

---

### Issue 11 — MEDIUM: `warn!` in `build_tls_connector` fires silently for cert load errors

**File:** [`crates/brassclaw_pg/src/pool.rs`](crates/brassclaw_pg/src/pool.rs) lines 61, 65

**Code:**
```rust
tracing::warn!("pg pool: error loading system root cert: {error}");
tracing::warn!("pg pool: skipping invalid system root cert: {error}");
```

**Fix:** Change to `tracing::debug!` — cert loading is an internal diagnostic. These fire during pool construction; if TLS setup is broken, the connection itself will fail with a clear error. The `warn!` noise is not actionable and corrupts UI.

---

### Issue 12 — LOW: `BTreeSet` used where `HashSet` is sufficient in `local_trigger_access.rs`

**File:** [`crates/brassclaw_reborn/src/local_trigger_access.rs`](crates/brassclaw_reborn/src/local_trigger_access.rs) line 10, 160

**Code:**
```rust
use std::collections::BTreeSet;
// ...
let allowed: BTreeSet<&str> = reconciliation.user_ids.iter().map(UserId::as_str).collect();
```

`BTreeSet` is O(n log n) to build and O(log n) per lookup. Only membership is checked (`contains`); no ordering or sorted iteration is needed. `HashSet` gives O(n) build and O(1) amortized lookup.

**Fix:** Replace `BTreeSet` with `HashSet` for the `allowed` set.

---

### Issue 13 — LOW: Defence-in-depth whitelist missing for dynamic table/column names in SQL

**File:** [`crates/brassclaw_reborn_composition/src/pg_auth_product_services.rs`](crates/brassclaw_reborn_composition/src/pg_auth_product_services.rs) lines 136, 178–183, 225, 241–242

**Analysis:** All call sites pass compile-time string literals for `table` and `filter_col` — no user input reaches these arguments today. However, as a generic helper, there is no guard preventing a future call site from accidentally passing a user-controlled value. The `filter_col` argument in particular is interpolated directly into a WHERE clause.

**Fix:** Add a debug-assertion whitelist check at the top of `get_record`, `put_record`, `delete_record`, and `list_records` to ensure `table` is one of the three known auth tables and `filter_col` is one of the known column names. This costs nothing in release builds but catches mis-use during development.

---

### Issue 14 — LOW: Deserialisation errors mapped to `BackendUnavailable` without logging

**File:** [`crates/brassclaw_reborn_composition/src/pg_auth_product_services.rs`](crates/brassclaw_reborn_composition/src/pg_auth_product_services.rs) lines 147, 252

**Code:**
```rust
serde_json::to_value(record).map_err(|_| AuthProductError::BackendUnavailable)?;
serde_json::from_value(json).map_err(|_| AuthProductError::BackendUnavailable)
```

If a record fails to serialise/deserialise (e.g., schema mismatch after a code update), it returns `BackendUnavailable` — indistinguishable from "database is down". The underlying error is discarded.

**Fix:** Log the serialisation error at `debug!` level before mapping it to `BackendUnavailable`.

---

## Round 2 Execution Order

- [x] **Step 10** — Fix `warn!` violations in `pool.rs` (cert errors → `debug!`, SSL warning → `eprintln!`) and `lib.rs` Drop (`warn!` → `eprintln!`) *(resolved)*
- [x] **Step 11** — Add `debug!` log to `load_config()` before silently returning empty config on DB error *(resolved)*
- [x] **Step 12** — Add `debug!` log to stale PID removal failure in `embedded_postgres/src/lib.rs` *(resolved)*
- [x] **Step 13** — Replace `BTreeSet` with `HashSet` in `local_trigger_access.rs` *(resolved)*
- [x] **Step 14** — Add debug-assertion whitelist in `pg_auth_product_services.rs` generic SQL helpers *(resolved)*
- [x] **Step 15** — Add `debug!` log to deserialisation errors in `pg_auth_product_services.rs` before mapping to `BackendUnavailable` *(resolved)*
- [x] **Step 16** — Run `cargo clippy -p brassclaw_pg -p brassclaw_embedded_postgres -p brassclaw_reborn -p brassclaw_reborn_composition --all-targets -- -D warnings` — zero warnings *(verified)*
- [x] **Step 17** — Run `cargo test -p brassclaw_pg -p brassclaw_embedded_postgres -p brassclaw_reborn -p brassclaw_reborn_composition` — all tests pass *(verified)*


## Round 3 Findings

### Issue 15 — MEDIUM: `debug_assert!` insufficient in `pg_auth_product_services.rs` — stripped in release builds

**File:** [`crates/brassclaw_reborn_composition/src/pg_auth_product_services.rs`](crates/brassclaw_reborn_composition/src/pg_auth_product_services.rs) lines 146, 179, 247, 267–271

Round 2 added `debug_assert!` guards that check `table` and `filter_col` against allowlists before SQL interpolation. However, `debug_assert!` is unconditionally stripped in `--release` builds. A future call site that accidentally passes a user-controlled string would bypass the guard silently in production.

**Fix:** Replace `debug_assert!` with a real `if !contains { return Err(...) }` guard so the check runs in all builds. This also provides a proper `AuthProductError` return path rather than a panic.

---

### Issue 16 — MEDIUM: Missing `user_id` index on `brassclaw_product_auth_*` tables

**File:** [`crates/brassclaw_reborn_composition/src/pg_auth_product_services.rs`](crates/brassclaw_reborn_composition/src/pg_auth_product_services.rs) lines 58–91 (inline DDL)

All three auth tables (`brassclaw_product_auth_accounts`, `brassclaw_product_auth_flows`, `brassclaw_product_auth_interactions`) define `user_id TEXT NOT NULL` but have only a `PRIMARY KEY (tenant_id, id)`. The `list_records` helper queries `WHERE tenant_id = $1 AND user_id = $2` — without a composite index on `(tenant_id, user_id)` this is a full-table scan on growing datasets.

**Fix:** Add `CREATE INDEX IF NOT EXISTS` for `(tenant_id, user_id)` on all three tables in the inline DDL.

---

### Issue 17 — LOW: Silent JSON serialisation fallback in `plan_library.rs`

**File:** [`crates/brassclaw_reborn_composition/src/plan_library.rs`](crates/brassclaw_reborn_composition/src/plan_library.rs) lines 196, 206

Two silent fallbacks:
- Line 196: `serde_json::to_vec(metrics).unwrap_or_else(|_| b"{}".to_vec())` — serialisation failure silently writes `{}`, losing all metrics data.
- Line 206: `serde_json::from_slice(&bytes).unwrap_or_default()` — deserialisation failure silently resets metrics to zero.

Neither failure is logged, so a serialisation bug or a corrupted metrics file is invisible.

**Fix:** Add `tracing::debug!` logs before the fallbacks so failures are observable.

---

### Issue 18 — LOW: Silent best-effort key delete in `llm_config_service.rs`

**File:** [`crates/brassclaw_reborn_composition/src/llm_config_service.rs`](crates/brassclaw_reborn_composition/src/llm_config_service.rs) line 950

```rust
let _ = self.keys.delete(&id).await;
```

The comment says "Best-effort: drop any stored key for the deleted provider" — the intent is correct, but a failure leaves an orphaned secret key in the store with no log. If the secret store is unavailable, the provider is deleted but its key persists silently.

**Fix:** Replace `let _ =` with `if let Err(e) = ... { tracing::debug!(...) }`.

---

## Round 3 Execution Order

- [x] **Step 18** — Replace `debug_assert!` with runtime `Err` guards in `pg_auth_product_services.rs` SQL helpers *(resolved)*
- [x] **Step 19** — Add `(tenant_id, user_id)` indexes to inline DDL in `pg_auth_product_services.rs` *(resolved)*
- [x] **Step 20** — Add `debug!` logs to JSON serialisation/deserialisation fallbacks in `plan_library.rs` *(resolved)*
- [x] **Step 21** — Add `debug!` log to best-effort key delete in `llm_config_service.rs` *(resolved)*
- [x] **Step 22** — Run `cargo clippy -p brassclaw_reborn_composition -p brassclaw_pg -- -D warnings` — zero warnings *(verified)*
- [x] **Step 23** — Run `cargo test -p brassclaw_reborn_composition` — 39 passed, 0 failed *(verified)*

---

## Round 4 Findings

### Issue 19 — MEDIUM: `tracing::warn!` in live background task loop (`trigger_poller.rs`)

**File:** [`crates/brassclaw_reborn_composition/src/trigger_poller.rs`](crates/brassclaw_reborn_composition/src/trigger_poller.rs) line 113

The `run_trigger_poller` function runs continuously as a `tokio::spawn`-ed background task. When `tick_once` fails it emits `tracing::warn!` — this is in the live loop body, not the shutdown path. Per AGENTS.md §67 background task errors should use `debug!` to avoid corrupting the terminal UI.

The shutdown path (`join_with_timeout`, lines 41–52) also uses `warn!` but fires only after the server has already begun tearing down, so those are borderline acceptable. The live-loop `warn!` at line 113 is the clear violation.

**Fix:** Change line 113 from `tracing::warn!` → `tracing::debug!`.

---

### Issue 20 — MEDIUM: Silent failure to zero secret key file in `migration.rs`

**File:** [`crates/brassclaw_reborn_composition/src/migration.rs`](crates/brassclaw_reborn_composition/src/migration.rs) line 1147

```rust
let _ = std::fs::write(path, zeros);   // zero-fill is best-effort
```

The comment says the zero-fill is defence-in-depth. If it fails, key material may remain on disk. There is no log of the failure — a permissions error here would be completely invisible to the operator.

**Fix:** Replace `let _ = std::fs::write(...)` with `if let Err(e) = ... { tracing::debug!(...) }`.

---

### Issue 21 — LOW: Best-effort secret deletes never log on failure (11 call sites)

**Files:**
- [`crates/brassclaw_reborn_composition/src/pg_auth_product_services.rs`](crates/brassclaw_reborn_composition/src/pg_auth_product_services.rs) line 467
- [`crates/brassclaw_reborn_composition/src/pg_auth_product_services.rs`](crates/brassclaw_reborn_composition/src/pg_auth_product_services.rs) lines 771, 775, 1401, 1404, 1485
- [`crates/brassclaw_reborn_composition/src/product_auth_durable/cleanup.rs`](crates/brassclaw_reborn_composition/src/product_auth_durable/cleanup.rs) lines 77, 80
- [`crates/brassclaw_reborn_composition/src/product_auth_durable/flows.rs`](crates/brassclaw_reborn_composition/src/product_auth_durable/flows.rs) lines 478, 483, 528, 533
- [`crates/brassclaw_reborn_composition/src/product_auth_durable/interactions.rs`](crates/brassclaw_reborn_composition/src/product_auth_durable/interactions.rs) line 284

All have adjacent comments explaining the best-effort intent — the account record no longer references the handle so orphaned material is unreachable. The intent is sound but a failure leaves orphaned secret-store entries with zero observability. Operators cannot detect a secret store that is accepting writes but rejecting deletes.

**Fix:** Replace each `let _ = self.secret_store.delete(...)` with `if let Err(e) = ... { tracing::debug!(...) }`.

---

### Issue 22 — LOW: Best-effort compensating deletes never log on failure

**Files:**
- [`crates/brassclaw_reborn_composition/src/extension_lifecycle.rs`](crates/brassclaw_reborn_composition/src/extension_lifecycle.rs) line 853
- [`crates/brassclaw_reborn_composition/src/available_extensions.rs`](crates/brassclaw_reborn_composition/src/available_extensions.rs) lines 934, 951

On error paths, rollback/compensating file deletes are attempted but silently swallowed. A failure leaves orphaned extension manifests or temp files.

**Fix:** Add `debug!` logs to compensating delete failures in these two files.

---

## Round 4 Execution Order

- [x] **Step 24** — Change `tracing::warn!` → `tracing::debug!` in `run_trigger_poller` live loop (line 113) *(resolved)*
- [x] **Step 25** — Log failure in `zero_and_delete` key-file zeroing in `migration.rs` *(resolved)*
- [x] **Step 26** — Add `debug!` logs to all best-effort `secret_store.delete` calls (11 sites, 3 files) *(resolved)*
- [x] **Step 27** — Add `debug!` logs to compensating deletes in `extension_lifecycle.rs` and `available_extensions.rs` *(resolved)*
- [x] **Step 28** — Run `cargo clippy -p brassclaw_reborn_composition --all-targets -- -D warnings` — zero warnings *(verified)*
- [x] **Step 29** — Run `cargo test -p brassclaw_reborn_composition` — 490 passed, 0 failed *(verified)*

---

## Non-Issues (Investigated and Cleared)

| Claim | Verdict |
|---|---|
| `edition = "2024"` in `brassclaw_embedded_postgres/Cargo.toml` is invalid | **False** — edition 2024 was stabilised in rustc 1.85; toolchain is 1.95.0. |
| V046 will fail on databases that already have the columns | **False** — all statements use `ADD COLUMN IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS`; idempotent by design. |
| V045 USING cast is unsafe | **Low risk** — the USING clause is idiomatic PostgreSQL; failure would only occur if existing TEXT values are not valid RFC-3339, which the application enforces at write time. |
| `tracing::warn!` at line 510 (Postgres shutdown failure) | **Acceptable** — fires only after WebUI has been torn down; UI is no longer rendering. |
| Multiple `rand` versions in `Cargo.lock` | **Maintenance debt** — dependency of transitive crates; cannot be fixed without upstream changes. Not actionable here. |
| Conditional indexes on `similarity_parent_id` / `replaces_id` | **Correct by design** — partial indexes with `WHERE IS NOT NULL` are the right choice for nullable UUID columns where null is the common case. |

---

## Round 5 Findings

### Issue 23 — MEDIUM: `tracing::info!` in startup code in `factory.rs`

**File:** [`crates/brassclaw_reborn_composition/src/factory.rs`](crates/brassclaw_reborn_composition/src/factory.rs) line 586

`tracing::info!("✅ Created extension_registry...")` fires during `build_local_dev()` — a startup path. Per AGENTS.md §67, `info!` corrupts the terminal UI. Changed to `tracing::debug!` with a structured `count` field (removing the emoji which belongs in operator-facing output, not debug logs).

---

### Issue 24 — MEDIUM: `tracing::warn!` in startup code in `factory.rs`

**File:** [`crates/brassclaw_reborn_composition/src/factory.rs`](crates/brassclaw_reborn_composition/src/factory.rs) line 1081

`tracing::warn!("local-dev: /memory is backed by InMemoryBackend...")` is an intentional operator-facing startup notice (same category as the `eprintln!` calls in `serve.rs`). Changed to `eprintln!` so it is always visible without corrupting the tracing-based UI.

---

### Issue 25 — LOW: Silent file removal on Windows ACL error in `factory.rs`

**File:** [`crates/brassclaw_reborn_composition/src/factory.rs`](crates/brassclaw_reborn_composition/src/factory.rs) line 1299

`let _ = std::fs::remove_file(path)` on the Windows-only cleanup path silently discards the removal result. Replaced with `if let Err(rm_err) = ... { debug!(...) }`.

---

### Issue 26 — MEDIUM: `tracing::warn!` in worker shutdown path in `runtime.rs`

**File:** [`crates/brassclaw_reborn_composition/src/runtime.rs`](crates/brassclaw_reborn_composition/src/runtime.rs) line 1105

Worker task cancellation during shutdown emits `warn!`. Cancellation is expected on a clean shutdown; changed to `debug!`.

---

### Issue 27 — MEDIUM: `tracing::warn!` in descendant cancellation loop in `runtime.rs`

**File:** [`crates/brassclaw_reborn_composition/src/runtime.rs`](crates/brassclaw_reborn_composition/src/runtime.rs) line 1249

The budget-cap log inside `cancel_descendant_runs` fires in the turn-coordinator loop. Changed from `warn!` → `debug!` per AGENTS.md §67.

---

### Issue 28 — MEDIUM: `tracing::warn!` in projection loop mapper in `turn_events.rs`

**File:** [`crates/brassclaw_reborn_composition/src/projection/turn_events.rs`](crates/brassclaw_reborn_composition/src/projection/turn_events.rs) line 644

`map_turn_event_projection_error` is called inside the SSE event-drain loop. `warn!` corrupts the terminal UI. Changed to `debug!`.

---

### Issue 29 — MEDIUM: `tracing::warn!` in auth read-model queries called from turn-coordinator loop

**File:** [`crates/brassclaw_reborn_composition/src/runtime/auth_interaction.rs`](crates/brassclaw_reborn_composition/src/runtime/auth_interaction.rs) lines 142, 185

`LocalDevAuthFlowRecordSource::flow_for_turn_gate` and `flows_for_owner` are called from the turn-coordinator's auth-interaction service (long-running loop). Both used `warn!`. Changed to `debug!`.

---

### Issue 30 — MEDIUM: `api_key: String` instead of `SecretString` in `OpenAiEmbeddings`

**File:** [`crates/brassclaw_reborn_composition/src/embedding_providers.rs`](crates/brassclaw_reborn_composition/src/embedding_providers.rs) line 232

`OpenAiEmbeddings` stored the OpenAI API key as a plain `String`. This means the key can appear in `{:?}` debug output of the struct. `SecretString` from the `secrecy` crate wraps the key so that `Debug` output is redacted. The call site already used `SecretString` for the config field; the provider struct was the only leaking layer. Changed `api_key: String` → `api_key: SecretString` and updated the header construction to call `.expose_secret()`.

---

### Issue 31 — MEDIUM: `tracing::warn!` in LLM retry loop in `brassclaw_llm`

**File:** [`crates/brassclaw_llm/src/retry.rs`](crates/brassclaw_llm/src/retry.rs) line 189

Every transient retry attempt emits `warn!`. Under load this spams the log; the retry is routine and operational, not anomalous. Changed to `debug!`.

---

### Issue 32 — MEDIUM: `tracing::info!` and `debug_assert!(false, ...)` in circuit breaker

**File:** [`crates/brassclaw_llm/src/circuit_breaker.rs`](crates/brassclaw_llm/src/circuit_breaker.rs) lines 163, 170

- Line 163: `tracing::info!` on HalfOpen→Closed recovery — this fires inside `record_success()` which is called from every LLM completion. Changed to `debug!`.
- Lines 170-175: `debug_assert!(false, ...)` to flag an invariant violation that is stripped in `--release`. Replaced with a `tracing::debug!` log and a comment explaining the graceful recovery, so the violation is observable in production without panicking.

---

### Issue 33 — LOW: `debug_assert!` in `from_trusted_static` / `from_trusted_string` in `brassclaw_turns`

**Files:**
- [`crates/brassclaw_turns/src/ids.rs`](crates/brassclaw_turns/src/ids.rs) line 308
- [`crates/brassclaw_turns/src/status.rs`](crates/brassclaw_turns/src/status.rs) line 178
- [`crates/brassclaw_turns/src/run_profile/refs.rs`](crates/brassclaw_turns/src/run_profile/refs.rs) lines 17, 23

These constructors are called exclusively with `&'static str` literals or deterministic format strings. The `debug_assert!` is the correct pattern here — all callers are compile-time verifiable. Added `// safety:` comments explaining the invariant so future reviewers understand why a runtime guard is not needed.

---

### Issue 34 — LOW: Silent progress event emission failure in `agent_loop` checkpoint

**File:** [`crates/brassclaw_agent_loop/src/executor/checkpoint.rs`](crates/brassclaw_agent_loop/src/executor/checkpoint.rs) line 104

`let _ = ctx.host.emit_loop_progress(event).await` — a best-effort operation but with no log on failure. Replaced with `if let Err(e) = ... { debug!(...) }`.

---

## Round 5 Execution Order

- [x] **Step 30** — Fix `tracing::info!` in `factory.rs` startup → `debug!` *(resolved)*
- [x] **Step 31** — Fix `tracing::warn!` in `factory.rs` startup → `eprintln!` *(resolved)*
- [x] **Step 32** — Log cleanup failure in `factory.rs` Windows ACL error path *(resolved)*
- [x] **Step 33** — Fix `tracing::warn!` in `runtime.rs` shutdown cancel path → `debug!` *(resolved)*
- [x] **Step 34** — Fix `tracing::warn!` in `runtime.rs` descendant cancel loop → `debug!` *(resolved)*
- [x] **Step 35** — Fix `tracing::warn!` in `projection/turn_events.rs` → `debug!` *(resolved)*
- [x] **Step 36** — Fix `tracing::warn!` in `runtime/auth_interaction.rs` → `debug!` *(resolved)*
- [x] **Step 37** — Fix `api_key: String` → `SecretString` in `embedding_providers.rs` *(resolved)*
- [x] **Step 38** — Fix `tracing::warn!` in `brassclaw_llm/src/retry.rs` retry loop → `debug!` *(resolved)*
- [x] **Step 39** — Fix `tracing::info!` + `debug_assert!(false)` in `circuit_breaker.rs` *(resolved)*
- [x] **Step 40** — Add safety comments to `debug_assert!` in `turns/ids.rs`, `status.rs`, `refs.rs` *(resolved)*
- [x] **Step 41** — Add `debug!` log to `emit_loop_progress` failure in `agent_loop/checkpoint.rs` *(resolved)*
- [x] **Step 42** — Run `cargo clippy` for all changed crates — zero warnings *(verified)*
- [x] **Step 43** — Run `cargo test` for all changed crates — all tests pass *(verified)*

---

## Non-Issues from Round 5 (Investigated and Cleared)

| Claim | Verdict |
|---|---|
| `unimplemented!()` in `brassclaw_llm/src/reasoning.rs` | **Test code** — `TruncatingLlm` is defined inside `#[cfg(test)]` module; no production impact. |
| `panic!()` in `brassclaw_turns/src/run_profile/milestones.rs:741` | **Test code** — `pretty()` is defined inside `#[cfg(test)]` module; no production impact. |
| `tracing::warn!` in `product_auth_serve/manual_token.rs` lines 79, 86 | **Acceptable** — explicit HTTP request handler error recovery, not a background loop. |
| `tracing::warn!` in `product_auth_serve/mod.rs` line 897 | **Acceptable** — explicit HTTP request handler, not a background loop. |

