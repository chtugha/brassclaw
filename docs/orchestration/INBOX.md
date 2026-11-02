# INBOX — Coordinator Message Queue

_Workers post BUILD_REQUEST and DONE here. Coordinator reads and processes._

---

## DONE — TASK-003

**From:** Coordinator (inline implementation)
**Time:** 2026-11-01T21:30:00Z
**Commits:** `942ff5c69`
**Result:** PASS
**Notes:**
- Removed `run_q1_validation`, `load_validator_program`, `run_validator_python_with_limits`,
  `ComponentContentFields`, `reviewed_content_fields` from `q1_orchestrator.rs`.
- Removed `AmbiguousValidator` and `InvalidValidator` variants from `Q1Error`.
- Kept `q1_db` helper (still used by retained path at lines 292, 331, 341).
- Removed unused `brassclaw_monty_host::utility::UtilityLimits` import.
- Removed `pub(crate) use review::Q1Candidate` re-export (no external namers).
- Removed legacy test in `validation_queue.rs` (`run_q1_validation_defers_when_no_recipe_seeded`).
- Cleaned stale comment in `retention_sweep.rs`.
- `cargo fmt --all -- --check`: CLEAN.
- `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings`: 0 warnings.
- `cargo test ... -- q1_orchestrator`: 4/4 pass.

---

## DONE — TASK-FIX2

**From:** Coordinator (inline fix, pre-TASK-001)
**Time:** 2026-11-01T20:50:00Z
**Commits:** `dc745d4fc`
**Result:** PASS
**Notes:**
- Moved `pool_for_validator` declaration inside `#[cfg(not(test))]` block in `runtime.rs`
  to eliminate unused-variable warning in test builds.
- `pool` changed to `pool.clone()` for `OwnedGlobalTaskFactory::new` (Arc clone, no semantic change).
- `cargo fmt --all -- --check`: CLEAN.
- `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings`: 0 warnings.

---

## DONE — TASK-004

**From:** Coordinator (inline implementation)
**Time:** 2026-11-01T21:45:00Z
**Commits:** `6672242b4`, `d2abc65b5` (fmt fix)
**Result:** PASS (non-integration compilation)
**Notes:**
- T5 (`run_q1_validation_retained_fails_for_empty_content`) added to
  `validator_retained_q1_tests.rs`.
- T1–T4 already present across `validation_queue.rs` and test file.
- T3–T5 require `--features integration` + live Postgres + Monty to run.
- `cargo fmt --all -- --check`: CLEAN.
- `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings`: 0 warnings.
- `cargo test -p brassclaw_skills`: 13/13 PASS.

---

## DONE — TASK-005

**From:** Coordinator (audit inline)
**Time:** 2026-11-02T00:10:00Z
**Result:** CLOSED — scope recalibrated
**Notes:**
- All L013–L025 bodies already use `def <export>(inputs):` preloadable format. Confirmed by direct source read.
- `python-preload/2` is a spec concept not yet a DB field in `NewPgPythonCode`; `dependency_registry: None` for all seeds.
- No code changes required. Task complete within current implementable scope.

---

## DONE — WP-1 / WP-2 / WP-4b / WP-4c (prior session screen builds)

**From:** Coordinator (processed from screen logs)
**Time:** 2026-11-01T08:16–08:25Z
**Result:** WP-1, WP-2: full PASS. WP-4b/4c: tests PASS, clippy stale (resolved by `965793bd9`).
**Notes:**
- WP-1: fmt CLEAN, clippy 0w, structural_validator_recipe_tests PASS, pg_validator_q1_evidence PASS.
- WP-2: fmt CLEAN, clippy 0w, routing_tests 4/4 PASS.
- WP-4b/4c: q1_admission unit tests PASS; clippy 3 dead_code warnings for pre-`965793bd9` source (resolved).


## DONE — L010 (builtin.file_exists)

**From:** Coordinator (inline implementation)
**Time:** 2026-11-02T00:08:30Z
**Commit:** `ba629233f`
**Result:** PASS

**Notes:**
- `builtin.file_exists` Rust primitive added: `FileExists` variant in `CodingCapabilityKind`;
  `file_exists()` handler in `coding/file.rs` (stat_optional path, returns `{kind, path}`).
- `FILE_EXISTS_CAPABILITY_ID`, `CodingCapabilityMetadata` entry, `file_exists.input.v1.json`
  schema added to `brassclaw_host_runtime`.
- `tool_file_exists_row()`, `ts_file_exists_row()` constructors added to `builtin_bootstrap.rs`;
  `let _ = tool_file_exists;` suppression removed; tool+toolskill added to `cat_read_file`
  and `cat_filesystem` children lists.
- `CAT_FILESYSTEM_OVERVIEW` updated to list `builtin.file_exists`.
- 6 test additions in `first_party_builtin_tools.rs` (capability list, effects, is_coding, etc.).
- `cargo fmt --all -- --check`: CLEAN
- `cargo clippy -p brassclaw_first_party_extensions -p brassclaw_host_runtime
  -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings`: 0 warnings
- `cargo test -p brassclaw_first_party_extensions`: 38/38 PASS
- `cargo test -p brassclaw_host_runtime --test first_party_builtin_tools`: 139/139 PASS
- Pre-existing `reborn_durable_restart_integration` failures (2) unrelated; not introduced.

---
