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
