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

- [ ] **Step 1** — Fix class-code mismatch in `COMPONENT_TABLES` and `class_label()` in `interceptor_config_service.rs`
- [ ] **Step 2** — Replace silent `unwrap_or` with error-logged skip in `interceptor_config_service.rs`
- [ ] **Step 3** — Fix logging violations in `serve.rs` (`warn!` → `eprintln!`, `info!` → `debug!`)
- [ ] **Step 4** — Add `#[deprecated]` attribute to `postgres_with_resolved_secret_master_key` in `input.rs`
- [ ] **Step 5** — Sanitize branch names in `sweepfix.py` commit message
- [ ] **Step 6** — Fix "seven" → "eight" in V046 migration comment
- [ ] **Step 7** — Improve safety comment on `.expect()` in `webui_auth.rs`
- [ ] **Step 8** — Run `cargo clippy --all --benches --tests --examples --all-features -- -D warnings` and confirm zero new warnings
- [ ] **Step 9** — Run `cargo test -p brassclaw_reborn_composition -p brassclaw_reborn_cli` to confirm no regressions

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
