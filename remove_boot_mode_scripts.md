# Plan: Remove Compiled-In Prompt Scripts and Harden Boot Sequence

*Verified against codebase at V084 migrations. All file paths and line numbers
are ground-truth references checked against the live source tree.*

---

## Problem Statement

BrassClaw has a **dual-source problem**: runtime behaviour can differ depending
on whether the binary's compiled-in fallback or the database row is in effect.
This creates five classes of risk:

1. **Silent divergence** — the production orchestrator load path silently falls
   back to the compiled-in `basic_mode.py` whenever the v1 `brassclaw_memory_docs`
   table is empty. The operator cannot observe or audit which version is running.
2. **V1 store holdover** — `prepare_monty_session()` calls
   `load_orchestrator_from_docs()`, which queries the old `brassclaw_memory_docs`
   table. The v3 `reborn_skills` table is bypassed entirely on every production turn.
3. **Corruption silence** — no mechanism detects whether a `source='system'` row
   has been mutated, corrupted, or partially written after seeding.
4. **Redundant text corpus** — twelve separate `include_str!()` prompt constants
   baked into the binary create a second parallel corpus that agents and operators
   cannot inspect, override, or validate through the component system.
5. **Boot ordering by convention** — component seeding happens after the pool is
   created, but nothing in the type system prevents code from running before
   migrations complete.

---

## Goal

After this plan is executed:

- `basic_mode.py`, `codeact_preamble.md`, `codeact_postamble.md`,
  `compaction_summarizer_fresh.md`, `failure_explanation.md`, `sempai_audit.md`,
  and the four subagent direction files (`general.md`, `researcher.md`,
  `explorer.md`, `coder.md`) are **DB rows**, seeded once by `builtin_bootstrap.rs`
  and never read as compiled-in fallbacks at runtime.
- `prepare_monty_session()` loads the orchestrator via the new
  `OrchestratorCodePort` engine-side port — the v3 path. The old
  `brassclaw_memory_docs`-based path is removed entirely.
- The failure-count / version-rollback mechanism is removed. Failure handling is
  Tier-2 degradation, not silent code rollback.
- A **content-checksum integrity check** runs at boot after seeding and halts
  the process on any `source='system'` row mismatch.
- A `brassclaw repair` CLI command force-overwrites all `source='system'` rows
  from compiled-in seed values (`ON CONFLICT DO UPDATE`), providing a
  deterministic, auditable recovery path.
- A `BootedDb` newtype enforces migration-before-seeding at the type level.

---

## Current State — What Exists Where

| Symbol | File | Lines | Problem |
|--------|------|-------|---------|
| `DEFAULT_ORCHESTRATOR` | `brassclaw_engine/src/executor/orchestrator.rs` | 70 | `include_str!` fallback — bypasses v3 |
| `ORCHESTRATOR_TITLE` | same | 73 | `pub const` — referenced by `is_protected_component_title()` in `prompt.rs:96`; **must be retired atomically with Step 7d** |
| `ORCHESTRATOR_TAG` | same | 76 | v1 MemoryDoc key — dead after this plan |
| `load_orchestrator()` | same | 433–457 | public fn that reads v1 `brassclaw_memory_docs` |
| `load_orchestrator_from_docs()` | same | 466–531 | failure-count rollback; v1 model |
| `record_orchestrator_failure()` | same | 534–593 | writes to v1 `brassclaw_memory_docs` |
| `MAX_FAILURES_BEFORE_ROLLBACK` | same | 299 | v1 rollback constant |
| `FAILURE_TRACKER_TITLE` | same | 302 | v1 tracking key |
| `prepare_monty_session()` | same | 1091–1127 | **production path**: calls `load_orchestrator_from_docs` at line 1112, bypassing v3 |
| `CODEACT_PREAMBLE` | `brassclaw_engine/src/executor/prompt.rs` | 30 | `include_str!` |
| `CODEACT_POSTAMBLE` | same | 33 | `include_str!` — also used as rfind marker at line 328 |
| `PREAMBLE_OVERLAY_TITLE` | same | 79 | v1 MemoryDoc overlay key — referenced by `is_protected_component_title()` at line 96 |
| `PROMPT_OVERLAY_TAG` | same | 82 | v1 overlay tag |
| `is_protected_component_title()` | same | 96–99 | security gate — references both `ORCHESTRATOR_TITLE` and `PREAMBLE_OVERLAY_TITLE`; **must not be deleted, must be rewritten** |
| `FAILURE_EXPLANATION_SYSTEM_PROMPT` | `brassclaw_loop_support/src/lib.rs` | 105–106 | `include_str!`; sole caller is `turn_events.rs:600` |
| `DEFAULT_SEMPAI_PERSONA` | `brassclaw_reborn/src/loop_driver_host.rs` | 2376–2377 | `include_str!`; gated `#[cfg(feature = "root-llm-provider")]`; callers at line 2170 and `interceptor_config_service.rs:246,254` |
| compaction summarizer | same | 1128 | inline `include_str!` passed to `default_host_managed_loop_compaction_port()` |
| `GENERAL_DIRECTION` etc. | `brassclaw_reborn/src/subagent/directions/mod.rs` | 22–25 | 4× `include_str!` |

---

## Architecture Note — The Port Pattern

`brassclaw_engine` is the innermost crate. It cannot depend on
`brassclaw_reborn_composition` (which owns `PgSkillStore`) or
`brassclaw_skills` (which owns `DbSkillStore`). There is no `SkillStore` trait
in the engine crate. **Do not invent one.**

The established v3 pattern for crossing this boundary is a **port trait**
defined in the engine crate and implemented in the composition crate — exactly
how `ComponentPort` (`composition_port.rs`) and `KohaiPort` (`kohai_port.rs`)
work. Both are exported via `crates/brassclaw_engine/src/executor/mod.rs` as:
```rust
pub mod composition_port;
pub mod kohai_port;
pub use composition_port::{ComponentPort, ComponentPortError};
pub use kohai_port::{KohaiPort, KohaiPortError};
```
All DB-backed lookups the engine needs at runtime go through a port trait.
This plan follows that pattern throughout.

## Boot Chain Location

**The seeding boot chain lives in `crates/brassclaw_reborn_composition/src/webui.rs`**,
not in `runtime.rs`. Specifically:

- `seed_builtin_providers` — `webui.rs:150` (runs first, before the Postgres block)
- `seed_builtin_host_components` — `webui.rs:200` (inside `#[cfg(feature = "postgres")]` block)
- `seed_builtin_components` — `webui.rs:214`
- `run_boot_integrity_check` — `webui.rs:226`

`runtime.rs` owns the `PersistentMontyDriver` construction (line ~2578) where
ports like `PgCompositionPort` and `PgKohaiPort` are wired — **that** is where
`PgOrchestratorCodePort` and `PgPromptBodyPort` are injected into the driver.
These are two distinct concerns. All `init_*` calls and the boot-chain seeding
go into `webui.rs`; driver construction wiring goes into `runtime.rs`.

---

## Migration Steps

### Step 0 — Add a `BootedDb` newtype (type-enforced boot ordering)

**File**: `crates/brassclaw_reborn_composition/src/booted_db.rs` *(new)*

```rust
/// Proof that Postgres migrations have completed for this boot cycle.
/// Constructed only by `run_migrations_and_return_booted_db()`.
/// Pass `&BootedDb` to any function that must not run before migrations.
#[must_use]
pub struct BootedDb {
    pub(crate) pool: brassclaw_pg::PgPool,
}

impl BootedDb {
    pub fn pool(&self) -> &brassclaw_pg::PgPool { &self.pool }
}
```

In `webui.rs`, replace the existing pattern where migrations are run and the
raw `PgPool` is immediately used for seeding with:

```rust
let booted_db: BootedDb = run_migrations_and_return_booted_db(pool).await?;
```

All subsequent seeding calls (`seed_builtin_host_components`,
`seed_builtin_components`, `run_content_integrity_check`,
`run_boot_integrity_check`) accept `&BootedDb`, not a raw `PgPool`. The
compiler prevents any seeding call from compiling before migrations run.

*No runtime behaviour change — this is a type-system guard only.*

---

### Step 1 — New migration V085: `content_checksum` column

**File**: `crates/brassclaw_pg/migrations/V085__component_content_checksum.sql` *(new)*

Scoped to the **three** prose-bearing component tables whose `source='system'`
rows carry behavioural text that is checksummed by this plan. JSONB-primary
tables (`reborn_recipes`, `reborn_extension_catalogues`, `reborn_actions`,
`reborn_docus`, etc.) and the `reborn_tools` descriptor table are excluded —
they carry no single seeded prose field to hash.

Note: `reborn_python_code` already has a `content_hash` column (for similarity
deduplication). The new `content_checksum` column is separate and serves the
boot integrity check only; the two columns are not interchangeable.

```sql
-- V085: add content_checksum (nullable SHA-256 hex) to prose-bearing component
-- tables. Populated by builtin_bootstrap seeder for source='system' rows.
-- NULL = not yet checked / user-authored row / JSONB-only table.
ALTER TABLE reborn_skills      ADD COLUMN IF NOT EXISTS content_checksum TEXT;
ALTER TABLE reborn_tool_skills ADD COLUMN IF NOT EXISTS content_checksum TEXT;
ALTER TABLE reborn_python_code ADD COLUMN IF NOT EXISTS content_checksum TEXT;
```

Zero-downtime `ALTER TABLE ADD COLUMN IF NOT EXISTS` — existing rows get NULL;
the seeder writes their checksums on the next boot.

---

### Step 2 — Checksum utility (shared internal module)

**File**: `crates/brassclaw_reborn_composition/src/checksum.rs` *(new)*

Both `builtin_bootstrap.rs` (seeder) and `content_integrity.rs` (Step 4) need
the same SHA-256 helper. Place it in a shared internal module:

```rust
//! Internal SHA-256 checksum helpers for component content integrity.

/// Compute the canonical SHA-256 hex of a component's prose field.
/// Applied to: `reborn_skills.body`, `reborn_tool_skills.content`,
/// `reborn_python_code.content`.
pub(crate) fn sha256_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(s.as_bytes()))
}
```

**`brassclaw_reborn_composition/Cargo.toml`** — add under `[dependencies]`:
```toml
sha2 = "0.10"
hex  = "0.4"
```

Expose from `lib.rs` as `pub(crate) mod checksum;`.

#### Adding `content_checksum` to the `New*` structs and their INSERT SQL

Add `content_checksum: Option<String>` to:

| Struct | File | Prose column | Current INSERT params | New param |
|--------|------|-------------|----------------------|-----------|
| `NewPgSkill` | `pg_skill_store.rs` | `body` | 12 (`$1`–`$12`) | `$13` |
| `NewPgToolSkill` | `pg_tool_skill_store.rs` | `content` | 17 (`$1`–`$17`) | `$18` |
| `NewPgPythonCode` | `pg_python_code_store.rs` | `content` | 14 (`$1`–`$14`) | `$15` |

For each struct, the new field defaults to `None` (no `Default` derive is
needed — just add `content_checksum: None` to any existing construction sites
that use struct literal syntax). Update the `INSERT INTO` SQL in the
corresponding store file to add `content_checksum` to the column list and `$N`
to the values list. The `ON CONFLICT DO NOTHING` path in `reborn_skills` and
`reborn_tool_skills` is unchanged — existing rows keep their stored checksum.
For `reborn_python_code`, the INSERT does not have ON CONFLICT; the new column
is simply part of the INSERT.

Add helper variants in `BootstrapStores` for `source='system'` seeding:

```rust
/// Like `upsert_skill` but writes `content_checksum` — used for
/// source='system' rows that participate in the boot integrity check.
async fn upsert_skill_with_checksum(
    &self,
    row: NewPgSkill,         // content_checksum: Some(checksum) pre-filled
    name: &str,
) -> Result<Uuid, SeedBuiltinBootstrapError>

async fn upsert_tool_skill_with_checksum(
    &self,
    row: NewPgToolSkill,     // content_checksum: Some(checksum) pre-filled
    name: &str,
) -> Result<Uuid, SeedBuiltinBootstrapError>

async fn upsert_python_code_with_checksum(
    &self,
    row: NewPgPythonCode,    // content_checksum: Some(checksum) pre-filled
    name: &str,
) -> Result<Uuid, SeedBuiltinBootstrapError>
```

These call the regular `insert` path; the checksum is already in the struct.
The existing `update_validation_status` call after `python_code.insert` is
unchanged. All other call sites continue to pass `content_checksum: None`.

---

### Step 3 — Compile-time expected checksum registry

Add to `builtin_bootstrap.rs`. Make `EXPECTED_CHECKSUMS` `pub(crate)` so
`content_integrity.rs` can import it directly:

```rust
use crate::checksum::sha256_hex;

/// Compile-time expected SHA-256 checksums for every source='system' component
/// covered by the boot integrity check. Key = component name; value = hex
/// digest of the seed constant. Derived automatically — no manual updates needed.
pub(crate) static EXPECTED_CHECKSUMS: std::sync::LazyLock<
    std::collections::HashMap<&'static str, String>
> = std::sync::LazyLock::new(|| {
    let mut m = std::collections::HashMap::new();
    m.insert("orchestrator:main",             sha256_hex(DEFAULT_ORCHESTRATOR_SEED));
    m.insert("codeact_preamble",              sha256_hex(CODEACT_PREAMBLE_SEED));
    m.insert("codeact_postamble",             sha256_hex(CODEACT_POSTAMBLE_SEED));
    m.insert("failure_explanation",           sha256_hex(FAILURE_EXPLANATION_SEED));
    m.insert("compaction_summarizer_fresh",   sha256_hex(COMPACTION_SUMMARIZER_SEED));
    m.insert("sempai_audit",                  sha256_hex(SEMPAI_AUDIT_SEED));
    m.insert("subagent:direction:general",    sha256_hex(DIRECTION_GENERAL_SEED));
    m.insert("subagent:direction:researcher", sha256_hex(DIRECTION_RESEARCHER_SEED));
    m.insert("subagent:direction:explorer",   sha256_hex(DIRECTION_EXPLORER_SEED));
    m.insert("subagent:direction:coder",      sha256_hex(DIRECTION_CODER_SEED));
    m
});
```

The `*_SEED` constants are embedded via `include_str!()` **only** in
`builtin_bootstrap.rs`. Runtime call sites are replaced with port-trait
queries or `OnceLock` accessors (Steps 6–10).

---

### Step 4 — Content integrity checker

**File**: `crates/brassclaw_reborn_composition/src/content_integrity.rs` *(new)*

```rust
use crate::builtin_bootstrap::EXPECTED_CHECKSUMS;
use crate::checksum::sha256_hex;

pub enum ContentIntegrityOutcome {
    Ok { checked: usize },
    Corrupted(Vec<ContentIntegrityMismatch>),
}

pub struct ContentIntegrityMismatch {
    pub table:    &'static str,
    pub name:     String,
    /// Expected: compile-time value from EXPECTED_CHECKSUMS.
    pub expected: String,
    /// Actual: re-computed SHA-256 from the DB row's prose field.
    pub actual:   String,
}
```

**`run_content_integrity_check(booted_db: &BootedDb) -> Result<ContentIntegrityOutcome, ...>`**

Queries the three prose tables from Step 1. Note the prose column name differs
per table: `reborn_skills` uses `body`; `reborn_tool_skills` and
`reborn_python_code` both use `content`:

```sql
-- reborn_skills:
SELECT name, body, content_checksum
  FROM reborn_skills
 WHERE source = 'system' AND content_checksum IS NOT NULL

-- reborn_tool_skills:
SELECT name, content, content_checksum
  FROM reborn_tool_skills
 WHERE source = 'system' AND content_checksum IS NOT NULL

-- reborn_python_code:
SELECT name, content, content_checksum
  FROM reborn_python_code
 WHERE source = 'system' AND content_checksum IS NOT NULL
```

For each row, performs a **two-way check**:

- Re-compute `sha256_hex(prose_field)` and compare to the stored `content_checksum`.
  Mismatch → row was mutated after seeding (DB corruption or manual edit).
- Compare the stored `content_checksum` to `EXPECTED_CHECKSUMS.get(name)`.
  Mismatch → binary was updated; `brassclaw repair` has not been run yet.

Both failure modes produce a `ContentIntegrityMismatch`. All mismatches are
collected before returning so the operator sees the complete picture in one boot.

**Boot wiring** in `webui.rs`, *after* `seed_builtin_components` (i.e. after
the existing `run_boot_integrity_check` block at line 226 — add immediately
before the doc-sync watcher block that follows):

```rust
match crate::content_integrity::run_content_integrity_check(&booted_db).await {
    Ok(ContentIntegrityOutcome::Ok { checked }) => {
        tracing::debug!(checked, "content integrity: all system components verified");
    }
    Ok(ContentIntegrityOutcome::Corrupted(mismatches)) => {
        for m in &mismatches {
            tracing::error!(
                table = m.table, name = %m.name,
                expected = %m.expected, actual = %m.actual,
                "CONTENT INTEGRITY FAILURE: system component corrupted or \
                 binary updated without repair. Run `brassclaw repair`."
            );
        }
        return Err(RebornBuildError::from(
            ContentIntegrityError::Corrupted(mismatches)
        ));
    }
    Err(e) => {
        tracing::error!(error = %e, "content integrity check failed; aborting boot");
        return Err(e.into());
    }
}
```

The WebUI bundle is not returned until this passes.

---

### Step 5 — `brassclaw repair` CLI command

**File**: `crates/brassclaw_reborn_cli/src/commands/repair.rs` *(new)*

```
brassclaw repair [--dry-run]
```

The normal seeder uses `ON CONFLICT DO NOTHING` — it will not overwrite a
corrupted row. The repair command requires a **separate SQL path** that
unconditionally overwrites:

```rust
/// Force-reseed all source='system' component rows from compiled-in seed
/// constants. Uses ON CONFLICT DO UPDATE to overwrite existing rows —
/// intentionally different from the normal seeder's DO NOTHING.
pub async fn repair_builtin_components(
    booted_db: &BootedDb,
    tenant_id: &str,
    dry_run: bool,
) -> Result<RepairReport, RepairError>
```

For each `*_SEED` constant, issue:
```sql
INSERT INTO reborn_skills (tenant_id, user_id, agent_id, project_id,
    name, description, body, class_code, consumer_tags,
    intent_examples, source, validation_status, content_checksum)
VALUES (...)
ON CONFLICT (tenant_id, user_id, agent_id, project_id, name)
DO UPDATE SET
    body              = EXCLUDED.body,
    content_checksum  = EXCLUDED.content_checksum,
    validation_status = 'validated',
    source            = 'system'
```

With `--dry-run`: executes inside a transaction that is always rolled back;
prints what would be overwritten without committing.

Behaviour:
1. Starts embedded Postgres and runs migrations (`BootedDb`).
2. Calls `repair_builtin_components(booted_db, tenant_id, dry_run)`.
3. Prints: `Restored N system components.` (or `Would restore N` in dry-run).

Register in `crates/brassclaw_reborn_cli/src/commands/mod.rs`:
- Add `pub(crate) mod repair;` to the module list.
- Add `Repair(repair::RepairArgs)` to the `Command` enum.
- Add the dispatch arm to `execute()`.

---

### Step 6 — Move `basic_mode.py` → class-10 DB row via `OrchestratorCodePort`

#### 6a — Seed constant in `builtin_bootstrap.rs`

```rust
/// Seed content for the class-10 Orchestrator component.
/// The ONLY place basic_mode.py is embedded via include_str!.
/// Written to DB on first boot; never read at runtime.
const DEFAULT_ORCHESTRATOR_SEED: &str =
    include_str!("../../brassclaw_engine/orchestrator/basic_mode.py");
```

New `seed_orchestrator()` function, called at the top of
`seed_builtin_components()` before all capability passes:

```rust
async fn seed_orchestrator(stores: &BootstrapStores) -> Result<(), SeedBuiltinBootstrapError> {
    let checksum = sha256_hex(DEFAULT_ORCHESTRATOR_SEED);
    stores.upsert_skill_with_checksum(
        NewPgSkill {
            tenant_id:         stores.tenant.clone(),
            user_id:           SEED_USER.to_string(),
            agent_id:          SEED_AGENT.to_string(),
            project_id:        SEED_PROJECT.to_string(),
            name:              "orchestrator:main".into(),
            description:       "Main orchestrator loop (class 10)".into(),
            body:              DEFAULT_ORCHESTRATOR_SEED.into(),
            class_code:        10,
            consumer_tags:     vec![],
            intent_examples:   serde_json::json!([]),
            source:            "system".into(),
            validation_status: "validated".into(),
            content_checksum:  Some(checksum.clone()),
        },
        "orchestrator:main",
    ).await
}
```

Note: All fields of `NewPgSkill` must be specified explicitly — the struct does
**not** derive `Default`. `..Default::default()` will not compile.

#### 6b — Define `OrchestratorCodePort` in `brassclaw_engine`

**File**: `crates/brassclaw_engine/src/executor/orchestrator_code_port.rs` *(new)*

```rust
//! Engine-side port for loading the class-10 Orchestrator component body.
//!
//! Follows the ComponentPort / KohaiPort pattern: trait defined here in the
//! engine crate, implemented in brassclaw_reborn_composition.
//! This is the only correct way to reach reborn_skills from brassclaw_engine.

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OrchestratorCodeError {
    #[error("orchestrator component not found in DB; run `brassclaw repair`")]
    NotFound,
    #[error("store error: {reason}")]
    Store { reason: String },
}

/// Port for loading the active Orchestrator code from the component store.
///
/// When `allow_self_modify` is false, returns the validated source='system'
/// row only. When true, prefers a validated operator-customised row if one
/// exists; otherwise falls back to the system row.
#[async_trait]
pub trait OrchestratorCodePort: Send + Sync {
    async fn load_orchestrator_code(
        &self,
        allow_self_modify: bool,
    ) -> Result<String, OrchestratorCodeError>;
}
```

Export from `crates/brassclaw_engine/src/executor/mod.rs` — add two lines:
```rust
pub mod orchestrator_code_port;
pub use orchestrator_code_port::{OrchestratorCodePort, OrchestratorCodeError};
```

#### 6c — Implement `PgOrchestratorCodePort` in `brassclaw_reborn_composition`

**File**: `crates/brassclaw_reborn_composition/src/pg_orchestrator_code_port.rs` *(new)*

Feature gate: `#[cfg(feature = "postgres")]` — orchestrator loading is needed
in all postgres builds, **not** restricted to `root-llm-provider`.

```rust
pub(crate) struct PgOrchestratorCodePort {
    pool: Arc<PgPool>,
    tenant_id: String,
}

#[async_trait]
impl OrchestratorCodePort for PgOrchestratorCodePort {
    async fn load_orchestrator_code(
        &self,
        allow_self_modify: bool,
    ) -> Result<String, OrchestratorCodeError> {
        let client = self.pool.get().await
            .map_err(|e| OrchestratorCodeError::Store { reason: e.to_string() })?;

        let row = if allow_self_modify {
            // Prefer validated non-system (operator-customised) row; fall back
            // to system row.
            client.query_opt(
                "SELECT body FROM reborn_skills
                  WHERE tenant_id  = $1
                    AND name       = 'orchestrator:main'
                    AND class_code = 10
                    AND validation_status = 'validated'
                  ORDER BY (source = 'system') ASC, -- non-system first
                           updated_at DESC
                  LIMIT 1",
                &[&self.tenant_id],
            ).await
        } else {
            client.query_opt(
                "SELECT body FROM reborn_skills
                  WHERE tenant_id  = $1
                    AND name       = 'orchestrator:main'
                    AND class_code = 10
                    AND source     = 'system'
                    AND validation_status = 'validated'
                  LIMIT 1",
                &[&self.tenant_id],
            ).await
        }
        .map_err(|e| OrchestratorCodeError::Store { reason: e.to_string() })?;

        row.map(|r| r.get::<_, String>(0))
           .ok_or(OrchestratorCodeError::NotFound)
    }
}
```

Wire `PgOrchestratorCodePort` into `PersistentMontyDriver` in `runtime.rs`
at the `PersistentMontyDriver::new(...)` call (~line 2578). The existing
`component_port` and `kohai_port` are gated on
`#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]`, but
`orchestrator_code_port` must be gated only on `#[cfg(feature = "postgres")]`
— the orchestrator loads regardless of which LLM provider is configured.

#### 6d — Rewrite `prepare_monty_session()` — the real production entry point

`prepare_monty_session` (lines 1091–1127) is called by
`persistent_monty_driver.rs` on every registry miss (fresh session). It
currently calls `load_orchestrator_from_docs` directly at line 1112.

New signature:
```rust
pub async fn prepare_monty_session(
    thread: &Thread,
    orchestrator_port: &Arc<dyn OrchestratorCodePort>,
    max_duration_override: Option<std::time::Duration>,
) -> Result<MontySession, EngineError>
```

Replace lines 1096–1112 (the `system_docs` fetch + `load_orchestrator_from_docs`
call) with:
```rust
let allow_self_modify = crate::runtime::self_modify_enabled();
let orchestrator_code = orchestrator_port
    .load_orchestrator_code(allow_self_modify)
    .await
    .map_err(|e| EngineError::OrchestratorLoad { reason: e.to_string() })?;
```

`PersistentMontyDriver` gains a new field. Unlike `component_port` and
`kohai_port` (which are `Option` because they are only available under
`root-llm-provider`), this field is **non-optional** — the compiled-in
fallback no longer exists:
```rust
orchestrator_code_port: Arc<dyn OrchestratorCodePort>,
```

Update the call site in `persistent_monty_driver.rs` (the registry-miss
branch, currently `prepare_monty_session(thread, Some(&self.store), max_duration_override)`)
to:
```rust
prepare_monty_session(thread, &self.orchestrator_code_port, max_duration_override)
```

#### 6e — Delete dead code from `orchestrator.rs`

**Critical ordering:** `ORCHESTRATOR_TITLE` (line 73) is imported by
`is_protected_component_title()` in `prompt.rs:96`. Steps 6e and 7d **must be
applied atomically in the same commit** — deleting `ORCHESTRATOR_TITLE` before
the rewrite in 7d causes a build failure.

After 7d is done, verify the compiler reports no remaining references to:

- `DEFAULT_ORCHESTRATOR` constant (line 70)
- `ORCHESTRATOR_TITLE` / `ORCHESTRATOR_TAG` constants (lines 73–76)
- `load_orchestrator()` (lines 433–457)
- `load_orchestrator_from_docs()` (lines 466–531)
- `record_orchestrator_failure()` (lines 534–593)
- `MAX_FAILURES_BEFORE_ROLLBACK` constant (line 299)
- `FAILURE_TRACKER_TITLE` constant (line 302)

Delete all of them.

#### 6f — Fix tests that used `DEFAULT_ORCHESTRATOR` directly

The grep shows `DEFAULT_ORCHESTRATOR` referenced at test lines 4182, 4201,
4230, 4238, 4247, 4331, 4333, 4334, 4346, 4348, 4349. All of these use it as
raw script text to test Monty VM compatibility — not as a production loader.
Replace **every** `DEFAULT_ORCHESTRATOR` inside `#[cfg(test)]` with a
test-local constant:

```rust
#[cfg(test)]
const BASIC_MODE_PY: &str =
    include_str!("../../orchestrator/basic_mode.py");
```

Delete these tests (they tested the now-removed v1 load path):
- `load_orchestrator_without_store_returns_default` (line 4385)
- `load_orchestrator_with_runtime_version` (line 4393)
- `load_orchestrator_picks_highest_version` (line 4413+)
- `record_and_reset_failures` (line 4537)
- `failure_count_resets_on_new_version` (line 4558)

Rewrite the `prepare_monty_session` tests (lines 4185–4238). These currently
pass `None` as the store argument — after the signature change they need a
mock port:

```rust
struct FixedOrchestratorCodePort(&'static str);

#[async_trait]
impl OrchestratorCodePort for FixedOrchestratorCodePort {
    async fn load_orchestrator_code(&self, _: bool)
        -> Result<String, OrchestratorCodeError>
    {
        Ok(self.0.to_string())
    }
}

// In the two existing prepare_monty_session tests:
let port = Arc::new(FixedOrchestratorCodePort(BASIC_MODE_PY));
let session = prepare_monty_session(&thread, &port, None).await;
```

Add a new test `orchestrator_loads_from_port` that asserts
`load_orchestrator_code(false)` returns the expected code body.

---

### Step 7 — Move `codeact_preamble.md` + `codeact_postamble.md` → DB rows

#### Architecture note for Step 7

**`build_codeact_system_prompt` and `build_codeact_system_prompt_with_docs` are
test-only — they are never called from production code.** Confirmed by grep:
the only file that calls them is `prompt.rs` itself, and every call site is
inside `#[cfg(test)]`. Similarly, `refresh_codeact_system_prompt` and
`upsert_codeact_system_prompt` — the functions that call
`codeact_system_prompt_suffix` (the `rfind(CODEACT_POSTAMBLE)` path) — are
also only called from tests.

**The preamble/postamble reach the LLM exclusively via the Kohai prefix
bundle.** `do_assemble_bundle` in `interceptor_config_service.rs` queries the
component tables (including `reborn_skills`) directly, formats all validated
rows into a flat bundle string via `do_format_bundle`, stores it in
`reborn_basic_prompt_store`, and `get_system_bundle` returns it cheaply per
turn as the system-message prefix. There is no call to
`build_codeact_system_prompt_inner` anywhere in this path.

Consequences:
- **No `PromptBodyPort` is needed.** The preamble/postamble are not loaded by
  the executor at runtime; they are read from `reborn_skills` by the bundle
  assembler which already queries the DB directly. A new engine-side port for
  this would add indirection to a path that doesn't exist.
- **No `OnceLock` / `init_prompt_bodies()` is needed.** Once the rows are
  seeded in DB (Step 7a), `do_assemble_bundle` picks them up automatically on
  the next bundle regeneration — no additional boot wiring is required.
- **`CODEACT_PREAMBLE` / `CODEACT_POSTAMBLE` and the functions that reference
  them are only used in tests.** They can be moved to `#[cfg(test)]` scope or
  deleted along with the functions that call them (Step 7c).
- **The `webui.rs` boot chain requires no changes for this step** beyond what
  Step 7a (seeding) already provides.

#### 7a — Seed constants

```rust
const CODEACT_PREAMBLE_SEED: &str =
    include_str!("../../brassclaw_engine/prompts/codeact_preamble.md");
const CODEACT_POSTAMBLE_SEED: &str =
    include_str!("../../brassclaw_engine/prompts/codeact_postamble.md");
```

Both seeded as `class_code: 10`, `source: "system"`, `validation_status: "validated"`.

After seeding, the next call to `regenerate_prefix` (operator-triggered or
test-triggered) will include these rows in the bundle. No boot-time
`init_*` call is needed — the bundle assembler queries DB directly.

#### 7b — ~~`PromptBodyPort`~~ — NOT NEEDED

~~**File**: `crates/brassclaw_engine/src/executor/prompt_body_port.rs`~~

This sub-step is removed. The preamble/postamble flow through the Kohai prefix
bundle, not through `PersistentMontyDriver` or `prepare_monty_session`. The
bundle assembler (`do_assemble_bundle` in `interceptor_config_service.rs`)
reads class-10 rows from `reborn_skills` directly — it does not go through a
port trait. No new port file, no new `PgPromptBodyPort`, no new field on
`PersistentMontyDriver`, no wiring in `runtime.rs`.

The "Files to Create / Modify" table entry for `prompt_body_port.rs` and
`pg_prompt_body_port.rs` is **removed**. The `runtime.rs` change description
for `PgPromptBodyPort` injection is also **removed**.

#### 7c — Retire `CODEACT_PREAMBLE` / `CODEACT_POSTAMBLE` constants in `prompt.rs`

Since `build_codeact_system_prompt_inner`, `codeact_system_prompt_suffix`,
`refresh_codeact_system_prompt`, and `upsert_codeact_system_prompt` are all
test-only call chains, the two `include_str!()` constants are not needed in
production scope. Move them inside `#[cfg(test)]`:

```rust
// BEFORE (lines 30, 33 — production scope):
const CODEACT_PREAMBLE:  &str = include_str!("../../prompts/codeact_preamble.md");
const CODEACT_POSTAMBLE: &str = include_str!("../../prompts/codeact_postamble.md");

// AFTER — move into the existing #[cfg(test)] mod at line 468:
#[cfg(test)]
const CODEACT_PREAMBLE:  &str = include_str!("../../prompts/codeact_preamble.md");
#[cfg(test)]
const CODEACT_POSTAMBLE: &str = include_str!("../../prompts/codeact_postamble.md");
```

`build_codeact_system_prompt_inner` is `pub(crate)` and uses `CODEACT_PREAMBLE`
/ `CODEACT_POSTAMBLE` in its body. Since it too is only called from tests,
move it (and the entire `else` branch) inside `#[cfg(test)]` scope as well, or
gate the constants with `#[cfg(test)]` and let the compiler enforce it — the
latter will break any future accidental production use loudly.

`STRUCTURED_TOOL_PREAMBLE` and `STRUCTURED_TOOL_POSTAMBLE` are inline raw
string literals — not `include_str!()` — and require no changes.

#### 7d — Fix `is_protected_component_title()` — keep, do not delete

`is_protected_component_title()` (`prompt.rs:96`) is the security gate called
at `orchestrator.rs:2808`. **Do not delete it.**

Current implementation:
```rust
pub fn is_protected_component_title(title: &str) -> bool {
    use crate::executor::orchestrator::ORCHESTRATOR_TITLE;
    title == ORCHESTRATOR_TITLE || title == PREAMBLE_OVERLAY_TITLE
}
```

Rewrite with inline string literals (eliminating the imported constants):
```rust
pub fn is_protected_component_title(title: &str) -> bool {
    // orchestrator:main (class 10) and codeact_preamble (class 10) require
    // Q1 LLM audit + Q2 manual validation before any memory_write is applied.
    title == "orchestrator:main" || title == "codeact_preamble"
}
```

`"codeact_preamble"` replaces `PREAMBLE_OVERLAY_TITLE` (`"prompt:codeact_preamble"`)
— the v3 name is used.

Update the test at `orchestrator.rs` (the `validate_component_protected_title_sets_validator_tag_and_audit_flag`
test at ~line 5997) to also assert `is_protected_component_title("codeact_preamble")`.

**This step must be applied in the same commit as Step 6e.** Deleting
`ORCHESTRATOR_TITLE` before this rewrite causes a build failure.

#### 7e — Remove v1 overlay functions from `prompt.rs`

Before deleting, verify all callers in the workspace. Then delete:
- `PREAMBLE_OVERLAY_TITLE` constant (line 79)
- `PROMPT_OVERLAY_TAG` constant (line 82)
- `load_prompt_overlay()` function (line 446)
- `extract_prompt_overlay()` function (line 452)
- `build_codeact_system_prompt()` async variant (line 114)
- `build_codeact_system_prompt_with_docs()` variant (line 140)

Update all call sites to use `build_codeact_system_prompt_inner()` directly,
passing `None` for overlay.

Delete the three overlay tests in `prompt.rs` (lines 483–576):
`prompt_with_overlay_appends_rules()`, `prompt_overlay_size_is_capped()`,
`prompt_ignores_wrong_project_overlay()`.

---

### Step 8 — Move `failure_explanation.md` → class-3 DB row

#### 8a — Seed constant

```rust
const FAILURE_EXPLANATION_SEED: &str =
    include_str!("../../brassclaw_loop_support/prompts/failure_explanation.md");
```

Seeded as `class_code: 3`, `name: "failure_explanation"`,
`source: "system"`, `validation_status: "validated"`.

#### 8b — Replace the `pub const` with a process-local `OnceLock`

`turn_events.rs:600` calls `brassclaw_loop_support::FAILURE_EXPLANATION_SYSTEM_PROMPT`
via a private wrapper. It is a synchronous call inside a `fn` — cannot await.

In `brassclaw_loop_support/src/lib.rs`, replace:
```rust
pub const FAILURE_EXPLANATION_SYSTEM_PROMPT: &str =
    include_str!("../prompts/failure_explanation.md");
```
with:
```rust
static FAILURE_EXPLANATION_PROMPT: OnceLock<String> = OnceLock::new();

/// Returns the failure explanation system prompt.
/// Panics if `init_failure_explanation_prompt` was not called at boot.
pub fn failure_explanation_system_prompt() -> &'static str {
    FAILURE_EXPLANATION_PROMPT
        .get()
        .expect("failure_explanation_prompt not initialised; \
                 call init_failure_explanation_prompt() at boot")
}

/// Initialise the failure explanation prompt from the DB.
/// Called once at boot via webui.rs after run_content_integrity_check.
pub fn init_failure_explanation_prompt(body: String) {
    let _ = FAILURE_EXPLANATION_PROMPT.set(body);
}
```

Add a private helper `load_system_skill_body` in `webui.rs`:
```rust
async fn load_system_skill_body(
    pool: &PgPool,
    tenant_id: &str,
    name: &str,
) -> Result<String, RebornBuildError> {
    let client = pool.get().await.map_err(/* pool error */)?;
    client.query_one(
        "SELECT body FROM reborn_skills
          WHERE tenant_id = $1 AND name = $2
            AND source = 'system' AND validation_status = 'validated'",
        &[&tenant_id, &name],
    ).await.map(|r| r.get::<_, String>(0)).map_err(/* pg error */)
}
```

In `webui.rs` boot chain (after content integrity check passes):
```rust
let fe_body = load_system_skill_body(booted_db.pool(), &host_tenant_id,
    "failure_explanation").await?;
brassclaw_loop_support::init_failure_explanation_prompt(fe_body);
```

`turn_events.rs:600` changes from:
```rust
fn failure_explanation_system_prompt() -> &'static str {
    brassclaw_loop_support::FAILURE_EXPLANATION_SYSTEM_PROMPT
}
```
to:
```rust
fn failure_explanation_system_prompt() -> &'static str {
    brassclaw_loop_support::failure_explanation_system_prompt()
}
```

---

### Step 9 — Move `compaction_summarizer_fresh.md` and `sempai_audit.md` → DB rows

#### 9a — Seed constants

```rust
const COMPACTION_SUMMARIZER_SEED: &str =
    include_str!("../../brassclaw_loop_support/prompts/compaction_summarizer_fresh.md");
const SEMPAI_AUDIT_SEED: &str =
    include_str!("../../brassclaw_engine/prompts/sempai_audit.md");
```

Note: `compaction_summarizer_update.md` is **out of scope** — it is not
currently a compiled-in constant.

Both seeded as `class_code: 10`, `source: "system"`, `validation_status: "validated"`.

#### 9b — Replace `DEFAULT_SEMPAI_PERSONA` — feature-gated throughout

`DEFAULT_SEMPAI_PERSONA` is defined with `#[cfg(feature = "root-llm-provider")]`.
The replacement must carry the same gate everywhere:

In `loop_driver_host.rs`:
```rust
#[cfg(feature = "root-llm-provider")]
static SEMPAI_PERSONA: OnceLock<String> = OnceLock::new();

#[cfg(feature = "root-llm-provider")]
pub fn sempai_persona() -> &'static str {
    SEMPAI_PERSONA
        .get()
        .expect("sempai_persona not initialised; call init_sempai_persona() at boot")
}

#[cfg(feature = "root-llm-provider")]
pub fn init_sempai_persona(body: String) {
    let _ = SEMPAI_PERSONA.set(body);
}
```

Both call sites (`loop_driver_host.rs:2170` and `interceptor_config_service.rs:246,254`)
become `sempai_persona()`.

In `webui.rs` boot chain, also gated:
```rust
#[cfg(feature = "root-llm-provider")]
{
    let body = load_system_skill_body(booted_db.pool(), &host_tenant_id,
        "sempai_audit").await?;
    brassclaw_reborn::loop_driver_host::init_sempai_persona(body);
}
```

#### 9c — Replace inline `include_str!` for compaction summarizer

In `loop_driver_host.rs` — no feature gate needed (compaction is independent
of the LLM provider):
```rust
static COMPACTION_SUMMARIZER: OnceLock<String> = OnceLock::new();

pub fn compaction_summarizer_prompt() -> &'static str {
    COMPACTION_SUMMARIZER
        .get()
        .expect("compaction_summarizer not initialised; \
                 call init_compaction_summarizer() at boot")
}

pub fn init_compaction_summarizer(body: String) {
    let _ = COMPACTION_SUMMARIZER.set(body);
}
```

In `webui.rs` boot chain:
```rust
let body = load_system_skill_body(booted_db.pool(), &host_tenant_id,
    "compaction_summarizer_fresh").await?;
brassclaw_reborn::loop_driver_host::init_compaction_summarizer(body);
```

---

### Step 10 — Move subagent direction files → DB rows

#### 10a — Seed constants

```rust
const DIRECTION_GENERAL_SEED: &str =
    include_str!("../../brassclaw_reborn/src/subagent/directions/general.md");
const DIRECTION_RESEARCHER_SEED: &str =
    include_str!("../../brassclaw_reborn/src/subagent/directions/researcher.md");
const DIRECTION_EXPLORER_SEED: &str =
    include_str!("../../brassclaw_reborn/src/subagent/directions/explorer.md");
const DIRECTION_CODER_SEED: &str =
    include_str!("../../brassclaw_reborn/src/subagent/directions/coder.md");
```

Each seeded as `class_code: 10`, `name: "subagent:direction:{id}"`,
`source: "system"`, `validation_status: "validated"`.

#### 10b — Replace `direction_prompt()` with `OnceLock` per direction

Replace the four `include_str!` constants in `directions/mod.rs` with four
`OnceLock<String>` statics. The public API `direction_prompt(id: DirectionId) -> &'static str`
is unchanged — callers need no modification.

```rust
static DIRECTION_GENERAL:    OnceLock<String> = OnceLock::new();
static DIRECTION_RESEARCHER: OnceLock<String> = OnceLock::new();
static DIRECTION_EXPLORER:   OnceLock<String> = OnceLock::new();
static DIRECTION_CODER:      OnceLock<String> = OnceLock::new();

pub fn direction_prompt(id: DirectionId) -> &'static str {
    match id {
        DirectionId::General    => DIRECTION_GENERAL.get()
            .expect("directions not initialised; call init_directions() at boot"),
        DirectionId::Researcher => DIRECTION_RESEARCHER.get()
            .expect("directions not initialised"),
        DirectionId::Explorer   => DIRECTION_EXPLORER.get()
            .expect("directions not initialised"),
        DirectionId::Coder      => DIRECTION_CODER.get()
            .expect("directions not initialised"),
    }
}

pub fn init_directions(
    general: String,
    researcher: String,
    explorer: String,
    coder: String,
) {
    let _ = DIRECTION_GENERAL.set(general);
    let _ = DIRECTION_RESEARCHER.set(researcher);
    let _ = DIRECTION_EXPLORER.set(explorer);
    let _ = DIRECTION_CODER.set(coder);
}
```

The existing test `direction_prompts_are_non_empty` must call `init_directions`
with the seed strings before asserting. The `direction_id_as_str_is_stable`
test needs no change.

```rust
#[test]
fn direction_prompts_are_non_empty() {
    // Test-local seed — no runtime fallback exists after this migration.
    const GENERAL_SEED: &str    = include_str!("general.md");
    const RESEARCHER_SEED: &str = include_str!("researcher.md");
    const EXPLORER_SEED: &str   = include_str!("explorer.md");
    const CODER_SEED: &str      = include_str!("coder.md");

    init_directions(
        GENERAL_SEED.to_string(),
        RESEARCHER_SEED.to_string(),
        EXPLORER_SEED.to_string(),
        CODER_SEED.to_string(),
    );
    assert!(!direction_prompt(DirectionId::General).trim().is_empty());
    assert!(!direction_prompt(DirectionId::Researcher).trim().is_empty());
    assert!(!direction_prompt(DirectionId::Explorer).trim().is_empty());
    assert!(!direction_prompt(DirectionId::Coder).trim().is_empty());
}
```

In `webui.rs` boot chain:
```rust
{
    let general    = load_system_skill_body(booted_db.pool(), &host_tenant_id,
        "subagent:direction:general").await?;
    let researcher = load_system_skill_body(booted_db.pool(), &host_tenant_id,
        "subagent:direction:researcher").await?;
    let explorer   = load_system_skill_body(booted_db.pool(), &host_tenant_id,
        "subagent:direction:explorer").await?;
    let coder      = load_system_skill_body(booted_db.pool(), &host_tenant_id,
        "subagent:direction:coder").await?;
    brassclaw_reborn::subagent::directions::init_directions(
        general, researcher, explorer, coder);
}
```

---

### Step 11 — Postgres-first boot: enforce `BootedDb` at every seeding call site

The seeding boot chain lives in `crates/brassclaw_reborn_composition/src/webui.rs`.
After this plan, the sequence within the `#[cfg(feature = "postgres")]` block is:

```
run_migrations_and_return_booted_db(pool)           → BootedDb
  (seed_builtin_providers runs earlier at webui.rs:150, before this block)
  → seed_builtin_host_components(&booted_db)        ← host.* stack  (was line 200)
  → seed_builtin_components(&booted_db)             ← incl. seed_orchestrator()
       └─ writes content_checksum for every system prose row
  → run_boot_integrity_check(&booted_db)            ← existing queue-consistency
  → run_content_integrity_check(&booted_db)         ← NEW: HARD ERROR on mismatch
  → init_failure_explanation_prompt(body)           ← failure_explanation
  → init_sempai_persona(body)  [cfg=root-llm-provider]
  → init_compaction_summarizer(body)                ← compaction_summarizer_fresh
  → init_directions(general, researcher, ...)       ← subagent directions
  [remaining webui.rs wiring: safety store, recipe store, interceptor config, …]
  → RebornWebuiBundle returned to caller
```

The `PgOrchestratorCodePort` injection happens separately in `runtime.rs` at
`PersistentMontyDriver::new(...)` (~line 2578) — wired at runtime construction
time, not at webui boot time. No `PgPromptBodyPort` is needed: the
preamble/postamble flow through the Kohai prefix bundle assembled by
`do_assemble_bundle` / `get_system_bundle`, which reads class-10 `reborn_skills`
rows directly.

The `BootedDb` type makes every seeding call compile-time-ordered after
migrations. The `OnceLock` initialisations make every runtime accessor safe —
a missing `init_*` call panics immediately on first use in tests, not silently
in production.

---

### Step 12 — Update `CLAUDE.md` and `AGENTS.md` to reflect the changed codebase

After all code changes are merged, update the two top-level documentation files.
This step is documentation-only.

#### Changes required in `CLAUDE.md`

1. **Orchestrator load path** — replace any description of
   `prepare_monty_session` loading via `load_orchestrator_from_docs` or
   `brassclaw_memory_docs` with: *"The orchestrator code is loaded from
   `reborn_skills` (class 10, name `orchestrator:main`) via
   `OrchestratorCodePort`, implemented by `PgOrchestratorCodePort` in
   `brassclaw_reborn_composition`."*

2. **Compiled-in fallback / dual-source** — remove any description of
   `DEFAULT_ORCHESTRATOR`, the failure-count rollback mechanism
   (`MAX_FAILURES_BEFORE_ROLLBACK`, `FAILURE_TRACKER_TITLE`), and
   `load_orchestrator_from_docs`. These no longer exist.

3. **Prompt body loading** — remove any description of
   `build_codeact_system_prompt` (async overlay variant) and
   `build_codeact_system_prompt_with_docs`. Both were test-only and are now
   deleted. The preamble/postamble are not loaded per-turn by the executor;
   they reach the LLM via the Kohai prefix bundle: `do_assemble_bundle` in
   `interceptor_config_service.rs` queries `reborn_skills` class-10 rows
   directly and includes them in the bundle stored in `reborn_basic_prompt_store`.
   No `OnceLock` / `init_prompt_bodies()` exists for this path.

4. **Boot sequence** — update the boot chain description to match the Step 11
   sequence. Note clearly that seeding lives in `webui.rs`, while driver port
   wiring lives in `runtime.rs`.

5. **Port table** — add `OrchestratorCodePort` (`orchestrator_code_port.rs`)
   alongside `ComponentPort` and `KohaiPort` in the engine-side port table.
   Do **not** add `PromptBodyPort` — it was removed from the plan (Step 7b);
   the preamble/postamble flow through the Kohai bundle, not a port.

6. **`brassclaw repair` command** — add to the CLI command reference list.

7. **V085 migration** — add to the migration history if one exists.

8. **`content_checksum` column** — document the new column on
   `reborn_skills`, `reborn_tool_skills`, `reborn_python_code`: nullable
   SHA-256 hex, populated for `source='system'` rows, verified at boot by
   `run_content_integrity_check`. Distinct from `content_hash` on
   `reborn_python_code` (which is used for similarity deduplication).

#### Changes required in `AGENTS.md`

1. **"Where to Work" table** — add entries:
   - `OrchestratorCodePort`: `crates/brassclaw_engine/src/executor/orchestrator_code_port.rs`
   - `PgOrchestratorCodePort`: `crates/brassclaw_reborn_composition/src/pg_orchestrator_code_port.rs`
   - `BootedDb` / `run_migrations_and_return_booted_db`: `crates/brassclaw_reborn_composition/src/booted_db.rs`
   - `run_content_integrity_check`: `crates/brassclaw_reborn_composition/src/content_integrity.rs`
   - `brassclaw repair`: `crates/brassclaw_reborn_cli/src/commands/repair.rs`
   - Seeding boot chain: `crates/brassclaw_reborn_composition/src/webui.rs` (not `runtime.rs`)
   - Driver port wiring: `crates/brassclaw_reborn_composition/src/runtime.rs` ~line 2578

2. **Seeding note** — update the bullet about `builtin_bootstrap.rs` to note:
   *"Orchestrator and system prompt components (`orchestrator:main`,
   `codeact_preamble`, `codeact_postamble`, `failure_explanation`,
   `compaction_summarizer_fresh`, `sempai_audit`, `subagent:direction:*`) are
   seeded here and carry `content_checksum` for boot-time integrity
   verification."*

3. **No compiled-in fallbacks rule** — add under "Repo-Wide Coding Rules":
   *"Do not introduce new `include_str!()` constants for behavioural prompts
   or scripts in production code paths. All prompt bodies must be seeded as
   `source='system'` DB rows via `builtin_bootstrap.rs` and loaded at boot via
   the appropriate port or `OnceLock` accessor. `include_str!()` is only
   permitted in `builtin_bootstrap.rs` seed constants and in `#[cfg(test)]`
   modules."*

4. **`boot_integrity.rs` vs `content_integrity.rs`** — note that
   `boot_integrity.rs` (existing, Phase N) handles queue-consistency checks
   and is distinct from `content_integrity.rs` (new, this plan) which handles
   SHA-256 prose checksums. Do not conflate them.

---

## What is NOT changed

| Item | Reason |
|------|--------|
| `default-system.md` | Already seed-to-disk; operator editable; correct as-is |
| `local_dev_capability_policy.toml` | Static config, not a prompt — stays compiled-in |
| `providers.json` | Static registry data, not a prompt — stays compiled-in |
| Filesystem migration SQL `include_str!` | Build-time schema loading, not runtime behaviour |
| `invocation_services.rs` shell/http source includes | Tool source inspection, not prompt |
| WebUI `INDEX_HTML_TEMPLATE` | Frontend asset, not component content |
| The `.md` + `.py` source files themselves | Stay on disk — seed source and test reference |
| `segment_reduction.py` | CPython reference only; never loaded into Monty VM |
| `STRUCTURED_TOOL_PREAMBLE` / `STRUCTURED_TOOL_POSTAMBLE` | Inline raw string literals in `prompt.rs` — not `include_str!()` files |
| `compaction_summarizer_update.md` | Not currently a compiled-in constant |
| `boot_integrity.rs` (`run_boot_integrity_check`) | Already correctly implemented (Phase N); adapt signature to accept `&BootedDb` |
| Other engine prompts (`architecture_prefix.md`, `memory_reasoning_synthesis.md`, `session_summary.md`, etc.) | Not currently compiled-in as runtime fallbacks |

---

## Files to Create / Modify

| File | Change |
|------|--------|
| `crates/brassclaw_pg/migrations/V085__component_content_checksum.sql` | **NEW** — `content_checksum TEXT` on `reborn_skills`, `reborn_tool_skills`, `reborn_python_code` |
| `crates/brassclaw_reborn_composition/src/checksum.rs` | **NEW** — `pub(crate) fn sha256_hex()` |
| `crates/brassclaw_engine/src/executor/orchestrator_code_port.rs` | **NEW** — `OrchestratorCodePort` trait + `OrchestratorCodeError` |
| `crates/brassclaw_engine/src/executor/mod.rs` | Add `pub mod` + `pub use` for `orchestrator_code_port` only (`prompt_body_port` removed — not needed) |
| `crates/brassclaw_reborn_composition/src/pg_orchestrator_code_port.rs` | **NEW** — `PgOrchestratorCodePort` impl (gate: `#[cfg(feature = "postgres")]`) |
| `crates/brassclaw_reborn_composition/src/booted_db.rs` | **NEW** — `BootedDb` newtype + `run_migrations_and_return_booted_db()` |
| `crates/brassclaw_reborn_composition/src/content_integrity.rs` | **NEW** — `run_content_integrity_check()` (distinct from existing `boot_integrity.rs`) |
| `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs` | Add `*_SEED` constants (pub(crate) `EXPECTED_CHECKSUMS`), `seed_orchestrator()`, `upsert_*_with_checksum()` helpers; add `content_checksum` to `New*` struct constructions |
| `crates/brassclaw_reborn_composition/src/pg_skill_store.rs` | Add `content_checksum: Option<String>` to `NewPgSkill`; update INSERT SQL 12→13 params |
| `crates/brassclaw_reborn_composition/src/pg_tool_skill_store.rs` | Add `content_checksum: Option<String>` to `NewPgToolSkill`; update INSERT SQL 17→18 params |
| `crates/brassclaw_reborn_composition/src/pg_python_code_store.rs` | Add `content_checksum: Option<String>` to `NewPgPythonCode`; update INSERT SQL 14→15 params |
| `crates/brassclaw_reborn_composition/src/webui.rs` | Thread `BootedDb`; add `run_content_integrity_check`; add all `init_*` calls; add `load_system_skill_body` helper |
| `crates/brassclaw_reborn_composition/src/runtime.rs` | Wire `PgOrchestratorCodePort` into `PersistentMontyDriver::new()` (~line 2578); `PgPromptBodyPort` is **not** wired here — removed from plan |
| `crates/brassclaw_reborn_composition/src/persistent_monty_driver.rs` | Add `orchestrator_code_port: Arc<dyn OrchestratorCodePort>` field (non-optional); update `prepare_monty_session` call |
| `crates/brassclaw_reborn_composition/Cargo.toml` | Add `sha2 = "0.10"`, `hex = "0.4"` |
| `crates/brassclaw_engine/src/executor/orchestrator.rs` | Delete v1 load path + fallback constants + failure tracker (Steps 6d/6e); rewrite `prepare_monty_session`; add `#[cfg(test)] const BASIC_MODE_PY`; replace all test `DEFAULT_ORCHESTRATOR` uses; delete/rewrite affected tests |
| `crates/brassclaw_engine/src/executor/prompt.rs` | Move `CODEACT_PREAMBLE`/`CODEACT_POSTAMBLE` to `#[cfg(test)]` (Step 7c); delete v1 overlay path (Step 7e); rewrite `is_protected_component_title()` with inline literals (Step 7d); no `OnceLock` added — preamble/postamble flow through Kohai bundle, not per-turn executor |
| `crates/brassclaw_loop_support/src/lib.rs` | Replace `FAILURE_EXPLANATION_SYSTEM_PROMPT` const with `OnceLock` + `init_failure_explanation_prompt()` + `failure_explanation_system_prompt()` |
| `crates/brassclaw_reborn_composition/src/projection/turn_events.rs` | Update `failure_explanation_system_prompt()` to call `brassclaw_loop_support::failure_explanation_system_prompt()` |
| `crates/brassclaw_reborn/src/loop_driver_host.rs` | Replace `DEFAULT_SEMPAI_PERSONA` (cfg-gated) + inline compaction `include_str!` with `OnceLock` accessors + `init_*` functions |
| `crates/brassclaw_reborn_composition/src/interceptor_config_service.rs` | Replace `DEFAULT_SEMPAI_PERSONA` at lines 246, 254 with `brassclaw_reborn::loop_driver_host::sempai_persona()` |
| `crates/brassclaw_reborn/src/subagent/directions/mod.rs` | Replace 4× `include_str!` constants with `OnceLock` + `init_directions()`; update test setup |
| `crates/brassclaw_reborn_cli/src/commands/repair.rs` | **NEW** — `brassclaw repair` subcommand |
| `crates/brassclaw_reborn_cli/src/commands/mod.rs` | Add `mod repair;`, `Repair` variant to `Command` enum, dispatch arm to `execute()` |
| `CLAUDE.md` | Update orchestrator load path, boot sequence, port table, CLI commands, migration history, `content_checksum` docs |
| `AGENTS.md` | Update "Where to Work" table, seeding note, add no-compiled-in-fallbacks rule, clarify `boot_integrity.rs` vs `content_integrity.rs` |

---

## Validation

1. `cargo clippy -p brassclaw_engine -p brassclaw_reborn_composition -p brassclaw_loop_support -p brassclaw_reborn -p brassclaw_reborn_cli --all-targets -- -D warnings`
   Zero warnings.

2. `cargo test -p brassclaw_engine` — old `load_orchestrator_*` and
   `record_orchestrator_failure` tests deleted; new `orchestrator_loads_from_port`
   test asserts the `OrchestratorCodePort` path; `prepare_monty_session` tests
   use `FixedOrchestratorCodePort`; `DEFAULT_ORCHESTRATOR` absent from
   non-test code paths.

3. `cargo test -p brassclaw_reborn_composition` — content integrity tests pass;
   seeder smoke test confirms `content_checksum IS NOT NULL` for all
   `source='system'` rows in the three prose tables.

4. Fresh install: empty DB → seeder runs → integrity check passes →
   all `init_*` calls succeed → `RebornWebuiBundle` returned → `orchestrator:main`
   row present in `reborn_skills` with non-NULL `content_checksum`.

5. Corruption test: update `body` on `orchestrator:main` without updating
   `content_checksum` → next boot halts with `CONTENT INTEGRITY FAILURE`.

6. Repair test: after corruption, run `brassclaw repair` → row overwritten
   (`ON CONFLICT DO UPDATE`) → next boot passes integrity check.

7. Security gate test: assert `is_protected_component_title("orchestrator:main")`
   and `is_protected_component_title("codeact_preamble")` both return `true`.

8. Integration: a full turn completes using the DB-loaded orchestrator;
   `grep -r 'DEFAULT_ORCHESTRATOR' crates/brassclaw_engine/src/executor/orchestrator.rs`
   finds no matches outside `#[cfg(test)]`.

---

## Risk Register

| Risk | Mitigation |
|------|-----------|
| DB unavailable on first boot | Embedded Postgres always starts before migrations; no external DB dependency |
| Seeder races with integrity check | `BootedDb` type makes this unrepresentable at compile time |
| Binary update triggers hard boot error before repair runs | Expected UX; document in release notes; `brassclaw repair --dry-run` previews what will be overwritten |
| `OnceLock` init forgotten in a new entry point | `expect()` names the missing call; panics immediately in integration tests, not silently in production |
| `CODEACT_POSTAMBLE` used in `rfind` search | `codeact_system_prompt_suffix` is test-only (only called via `refresh_codeact_system_prompt`, which is only called from tests); no runtime risk — the constant stays in `#[cfg(test)]` scope |
| `interceptor_config_service.rs` fallback silently bypassed | Both call sites (lines 246, 254) are updated to `sempai_persona()` |
| `upsert_*` SQL changes break existing seeder call sites | `content_checksum: Option<String>` defaults `None`; all existing call sites set it explicitly — no `Default` derive needed |
| `prepare_monty_session` test double needs `async_trait` | `FixedOrchestratorCodePort` is a two-line struct with `#[async_trait]`; no dependency friction |
| `DEFAULT_SEMPAI_PERSONA` feature gate missed | All replacement code (`OnceLock`, accessor, init fn, boot call) carries the same `#[cfg(feature = "root-llm-provider")]` gate |
| `reborn_python_code.content_hash` confused with `content_checksum` | Called out explicitly in Step 1; `content_hash` is for similarity deduplication; `content_checksum` is for boot integrity — two distinct columns |
| `direction_prompts_are_non_empty` test broken | Test is updated with inline `include_str!` seed setup before assertions |
| Steps 6e and 7d applied in wrong order (build break) | Plan explicitly requires atomic commit |
| `PgOrchestratorCodePort` inadvertently gated on `root-llm-provider` | Feature gate is `#[cfg(feature = "postgres")]` only — explicitly noted in Steps 6c and runtime.rs wiring |
| Boot chain location confusion (runtime.rs vs webui.rs) | Architecture Note section explains the split: seeding/`init_*` in `webui.rs`; driver port injection in `runtime.rs` |
