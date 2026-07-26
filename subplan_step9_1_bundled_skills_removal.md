# Subplan: Step 9.1 — Delete On-disk SKILL.md Discovery Code

## Status: ✅ IMPLEMENTED

`bundled_skills` module gated `#[cfg(not(feature = "skills-db"))]` in
`lib.rs:34`. `build.rs:38` detects `CARGO_FEATURE_SKILLS_DB` and gates
skill embedding. `skill_listing.rs` bundled-merge gated. `factory.rs` boot
call gated. `bundled_skills.rs` retained but only compiled without `skills-db`.
All steps 1–7 complete.

## Goal (historical)
Remove the v1 filesystem-backed skill path:
- `crates/brassclaw_reborn_composition/src/bundled_skills.rs` (compile-time embedded skill installer)
- The `embed_reborn_skills()` + `embed_migrated_skills_catalog()` calls in `build.rs`
- The `skill_listing.rs` coupling to `bundled_reborn_skill_summaries()`
- The `factory.rs` call to `ensure_bundled_reborn_skills_installed()`
- **NOT** `brassclaw_skills::registry` (SkillRegistry) — this is still used by local-dev skill management (install/remove user skills). Only the bundled embedded blob migration is removed.

## Current callers

### 1. `factory.rs:623`
```rust
crate::bundled_skills::ensure_bundled_reborn_skills_installed(&root).await?;
```
**Purpose:** At local-dev boot, extract embedded skill bundles (from build-time JSON) to the
`/projects/system/skills/` VFS. These are the v1 SKILL.md files (coding, commit, code-review, etc.).
**What replaces it:** `run_skill_import` (from `skill_import.rs`) via the `skills-db` feature,
which reads SKILL.md files from `skills/*/SKILL.md` and inserts them into `reborn_skills` DB.
On non-postgres/local-dev builds, the filesystem skills are still discoverable by `SkillRegistry`
via the `skill_filesystem` mount. The VFS extraction step is only needed for legacy DB-less reads.
**Removal strategy:** Gate the call behind `#[cfg(not(feature = "skills-db"))]` — when `skills-db`
is active the DB importer is the authoritative source; when `skills-db` is inactive the bundled
skills are still needed for the legacy retrieval path.

### 2. `skill_listing.rs:7,23-39`
```rust
use crate::bundled_skills::bundled_reborn_skill_summaries;
let bundled_skills = bundled_reborn_skill_summaries()?;
```
**Purpose:** `list_reborn_local_skills()` merges bundled skill summaries into the list result
so that system skills always appear even if not yet extracted to VFS.
**Replacement:** With `skills-db` active the DB is the authoritative source; the listing
should query `DbSkillStore` instead. For the non-db path, `SkillRegistry` discovers them from
the filesystem mount which includes the system skills directory.
**Removal strategy:** Gate the `bundled_reborn_skill_summaries` path behind
`#[cfg(not(feature = "skills-db"))]`.

### 3. `build.rs` — `embed_reborn_skills()` + `embed_migrated_skills_catalog()`
Both functions produce compile-time artifacts referenced by `bundled_skills.rs` via
`include_str!(concat!(env!("OUT_DIR"), "/..."))`. If `bundled_skills.rs` is removed, these
artifacts are no longer referenced. The `build.rs` calls can be removed entirely (or kept
only for non-db builds as dead no-ops).

## Steps

### Step 1 — Gate `ensure_bundled_reborn_skills_installed` in `factory.rs`
In `build_local`:
- Wrap the call in `#[cfg(not(feature = "skills-db"))]`
- When `skills-db` is active, call `crate::skill_import::run_skill_import(...)` instead
  (this is already done in the serve path via `spawn_q1_validation_sweep` — at boot we want
  an eager one-shot import so skills are in the DB before the first turn)
- Look at where `skills-db` boot import is wired: check if there's already a `run_skill_import`
  call in the serve path or factory.

### Step 2 — Gate `bundled_reborn_skill_summaries` in `skill_listing.rs`
- When `skills-db` is active: `list_reborn_local_skills` should not merge bundled summaries
  (the DB is authoritative). Remove the merge block or gate it.
- When `skills-db` is NOT active: keep existing behavior (bundled summaries merged).

### Step 3 — Gate `bundled_skills` module in `lib.rs`
```rust
#[cfg(not(feature = "skills-db"))]
mod bundled_skills;
```

### Step 4 — Gate `build.rs` catalog embedding behind `not(skills-db)`
In `build.rs`, wrap `embed_reborn_skills()` + `embed_migrated_skills_catalog()` calls
inside a runtime check: if neither output file is referenced by any compiled-in source
(because `bundled_skills.rs` is cfg-gated out), the build script can be simplified to
emit empty files when `skills-db` feature is set. Cargo doesn't pass feature flags to
`build.rs` — use `cargo:rustc-cfg=feature="skills-db"` detection via env var
`CARGO_FEATURE_SKILLS_DB` which Cargo sets automatically.

### Step 5 — Check where skill_import is called at boot
Look in `serve.rs`, `runtime.rs`, `factory.rs` for the `run_skill_import` call. If not
present at local-dev boot, add one-shot eager import after local-dev path setup.

### Step 6 — Run clippy + tests
```bash
cargo clippy -p brassclaw_reborn_composition --all-targets --all-features -- -D warnings
cargo test -p brassclaw_reborn_composition
cargo test -p brassclaw_skills
```

### Step 7 — Mark checkup.md Step 9.1, commit and push

## Files to touch
- `crates/brassclaw_reborn_composition/build.rs` — gate embedding behind `CARGO_FEATURE_SKILLS_DB`
- `crates/brassclaw_reborn_composition/src/lib.rs` — gate `mod bundled_skills`
- `crates/brassclaw_reborn_composition/src/factory.rs` — gate boot call, add skill_import boot call
- `crates/brassclaw_reborn_composition/src/skill_listing.rs` — gate bundled merge
- `crates/brassclaw_reborn_composition/src/bundled_skills.rs` — kept but only compiled without skills-db
