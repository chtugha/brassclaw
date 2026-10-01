# CLAUDE.md / AGENTS.md Cleaning — Pass 1

**Source:** Post-code-change doc audit (v3 skills store architecture session)
**Tracking:** Proposals P-01 through P-07 from the audit artifact `doc_audit_proposals`

All steps in this plan are pure documentation edits to `CLAUDE.md`.
No code changes. No migrations. No Cargo changes.

Each step includes: the exact file + line range, a verification note (how the
current codebase state was confirmed), and the precise text change to apply.

---

## Step 1 — P-02: Remove `handle_execute_action` negative reference (CLAUDE.md §Runtime security)

**Verified:** The symbol `handle_execute_action` does not exist anywhere in the current
codebase. `grep -r handle_execute_action crates/` returns zero matches. It was a v2-era
per-call security wrapper removed before v3. The opening sentence of the Runtime security
section names a removed symbol unnecessarily — engineers who never knew v2 gain nothing
from the negative reference; engineers who do know v2 already know it is gone.

**File:** `CLAUDE.md`
**Current lines 350–352:**
```
There is **no universal per-call security wrapper** (the old
`handle_execute_action` policy/lease/gate/event wrapper is retired as a
per-call babysitter). Security is **mode-driven + operator-toggleable**:
```

**Change to:**
```
There is **no universal per-call security wrapper**. Security is
**mode-driven + operator-toggleable**:
```

---

## Step 2 — P-01: Remove `ThreadManager → ExecutionLoop` tombstone (CLAUDE.md §Monty VM Settings)

**Verified:** Neither `ThreadManager`, `ExecutionLoop`, nor `execute_orchestrator` appear
anywhere in `crates/` today. The parenthetical tombstone "(The old `ThreadManager` →
`ExecutionLoop` → `execute_orchestrator` threading path was Model-A and is retired.)" is
a historical aside that adds no actionable guidance.

**File:** `CLAUDE.md`
**Current line 517:**
```
`PgMontyVmSettingsStore` reads/writes `reborn_monty_vm_settings` (V034 migration). `max_duration_secs` bounds the Orchestrator's main-process turn. The legacy `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS` env var is a DB-less fallback only. (The old `ThreadManager` → `ExecutionLoop` → `execute_orchestrator` threading path was Model-A and is retired.)
```

**Change to:**
```
`PgMontyVmSettingsStore` reads/writes `reborn_monty_vm_settings` (V034 migration). `max_duration_secs` bounds the Orchestrator's main-process turn. The legacy `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS` env var is a DB-less fallback only.
```

---

## Step 3 — P-03: Update "v1 migration not yet implemented" limitation (CLAUDE.md §Current Limitations)

**Verified:** `crates/brassclaw_reborn_composition/src/migration.rs` exists and implements
the full v1→v3 migration pipeline in Steps 3–7:
- **Step 3:** `config.toml` → `brassclaw_config` rows
- **Step 4:** `providers.json` → `brassclaw_llm_providers`
- **Step 5:** `sempai_provider.json` → `brassclaw_config`
- **Step 6:** Secrets master key (`.reborn-local-dev-secrets-master-key` → DB)
- **Step 7:** `reborn-local-dev.db` (libSQL) → Postgres tables (safety_config,
  token_settings, memory_docs, root_filesystem, capability_permissions, hooks,
  triggers, local_reborn_access)

Additionally `crates/brassclaw_reborn_composition/src/component_import.rs` migrates
legacy `brassclaw_memory_docs` MemoryDoc rows → class-specific component tables.

The limitation as written ("v1 config, DB, settings, and secrets migration not yet
implemented") is **factually incorrect** — all of these are implemented.

**File:** `CLAUDE.md`
**Current line 880:**
```
2. Reborn runtime: v1 config, DB, settings, and secrets migration not yet implemented
```

**Change to:**
```
2. Reborn runtime: v1→v3 data migration implemented (`migration.rs` Steps 3–7: config.toml, providers.json, secrets master key, libSQL DB → Postgres). Long-lived daemon/service installation not yet supported (see item 1).
```

---

## Step 4 — P-04: Remove struck-through WIT bindgen limitation (CLAUDE.md §Current Limitations)

**Verified:** Item 4 has been fully struck through in the source and annotated "removed
in Phase 4". A struck-through completed item in an active limitations list is noise.
The surrounding items are renumbered accordingly.

**File:** `CLAUDE.md`
**Current line 882:**
```
4. ~~WIT bindgen: auto-extract tool schema from WASM is stubbed~~ — removed in Phase 4; tool schemas come from native Extension Manifest v2 / MCP server introspection
```

**Change:** Delete this line entirely. Renumber subsequent items (old 5→4, 6→5, 7→6, 8→7).

**Resulting list after deletion:**
```
1. Reborn runtime: long-lived daemon/service installation not yet supported
2. Reborn runtime: v1→v3 data migration implemented (see updated text above)
3. MCP: no streaming support; stdio/HTTP/Unix transports all use request-response
4. Built tools get empty capabilities; no UX for granting access
5. No tool versioning or rollback
6. Observability: only `log` and `noop` backends (no OpenTelemetry)
7. `brassclaw` not yet included in cargo-dist release artifacts (see issue #3483)
```

---

## Step 5 — P-05: Fix `brassclaw_engine` project structure description (CLAUDE.md §Project Structure)

**Verified:** The description "Engine v2: planning, CodeAct, tool loop" uses two stale
v2-era terms ("v2" as a generation label, "CodeAct" as the execution model name). In v3
the execution model is Monty (Python orchestrator) calling Rust Tools via `host.<tool>()`.
"CodeAct" is not used in any v3 doc or source file as a current concept label.

**File:** `CLAUDE.md`
**Current line 685:**
```
│   ├── brassclaw_engine/           # Engine v2: planning, CodeAct, tool loop
```

**Change to:**
```
│   ├── brassclaw_engine/           # Execution engine: intent matching, IBS, orchestrator executor, tool dispatch
```

---

## Step 6 — P-06: Confirm and keep `crates/brassclaw_reborn_composition/CLAUDE.md` in Module Specs table

**Verified:** `crates/brassclaw_reborn_composition/CLAUDE.md` **exists**. The file was
confirmed present by directory listing. The Module Specs table entry on line 738 is
therefore **correct as written** — no change required for this item.

*(This step is a no-op but is recorded here to close out the proposal.)*

---

## Step 7 — P-07: Fix `brassclaw_pg` migration range in project structure (CLAUDE.md §Project Structure)

**Verified:** `crates/brassclaw_pg/migrations/` contains migrations V000–V084. The
highest file is `V084__reborn_component_catalog_view.sql`. The project structure comment
says "V000–V026" which is the range that existed at the time of initial documentation —
59 additional migrations have been added since.

**File:** `CLAUDE.md`
**Current line 680:**
```
│   ├── brassclaw_pg/               # Postgres pool, migration runner, SQL migrations V000–V026
```

**Change to:**
```
│   ├── brassclaw_pg/               # Postgres pool, migration runner, SQL migrations V000–V084
```

---

## Application Order

Apply steps in order 1 → 7. Steps 3 and 4 both touch the §Current Limitations list
and must be applied together to keep numbering consistent: apply Step 3 first (text
change to item 2), then Step 4 (delete the struck-through item 4 and renumber 5–8).
All other steps are independent.

## Verification after applying

```bash
# Confirm no stale symbol references remain in CLAUDE.md
grep -n "handle_execute_action\|ThreadManager\|ExecutionLoop\|execute_orchestrator\|CodeAct preamble\|V000--V026\|WIT bindgen" CLAUDE.md
# Expected: zero matches
```
