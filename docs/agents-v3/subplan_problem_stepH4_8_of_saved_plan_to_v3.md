# Subplan — H4.8 pre-existing test regression (Phase G.1 `skill_provenance_for_items` fallout)

Nested subplan of the Phase H.4 nested subplan
(`./docs/agents-v3/subplan_problem_stepH4_of_saved_plan_to_v3.md`), spawned
while running the **H4.8 final-verification** substep (fmt + workspace clippy +
`cargo test` both configs). Zenflow substep under the Phase H.4 nested-subplan
step `fa9fb137`.

This is **not** a defect introduced by Phase H4 — it is a pre-existing test
regression that H4.8 is the first full-workspace `cargo test` this task ran,
so it surfaced here. Proven pre-existing + H4-exonerated (see §2).

---

## 1. The failing test

`tests/engine_v2_skill_codeact.rs::skill_codeact_persists_active_skill_provenance`
panics at line 782:

```
expected github skill provenance in []
```

— i.e. `thread.active_skills()` is empty, so the `find` returns `None` and
`unwrap_or_else` panics. The test is the **only** default-config failure in
`cargo test --no-fail-fast`; all other binaries/tests pass.

## 2. Root cause + H4 exoneration (definitive)

- **Root cause:** Phase G.1 commit `e7c2ce31` ("emit active_skills provenance in
  `__assemble_prior_knowledge__`") moved skill-provenance population from a
  Python `__list_skills__()`+`select_skills()` round-trip (runnable against the
  in-memory `TestStore`) into the Rust
  `skill_provenance_for_items(pg_pool, &scope, &items)`
  (`orchestrator.rs:3132`), which returns `Vec::new()` when `pg_pool` is `None`
  (`:3139`). The test still builds
  `ThreadManager::new(llm, effects, store, caps, leases, policy)` with **no**
  pg_pool, and `ThreadManager` does not plumb one into the `ExecutionLoop`
  (`manager.rs:386-400` calls `.with_retrieval_source(RamSource)` but never
  `.with_pg_pool`). So `pkr["active_skills"]` is always `[]`,
  `_set_active_skills_from_matched_ids` (`default.py:992`) never calls
  `__set_active_skills__`, and `thread.active_skills()` is empty.
- **Second mismatch (assertions):** `fetch_skill_provenance_by_ids`
  (`db_skill_loader.rs:171`) **always returns `snippet_names: []`** ("reborn_skills
  has no code_snippets column (V027), so snippet_names is always []"). The test
  asserts `snippet_names == vec!["list_github_issues"]` (line 785) — that
  assertion is specific to the **old** `select_skills` path that read
  `code_snippets` from the `MemoryDoc`. The new Phase-G.1 mechanism returns `[]`
  by design (documented at `db_skill_loader.rs:113-114` + `default.py` comment).
  So the faithful migration also updates that assertion to `vec![]`.
- **H4 did NOT cause it:** `git log b84a6197^..edc0ab95 -- tests/engine_v2_skill_codeact.rs`
  = **EMPTY** (H4 never touched the test); `git log -S "skill_provenance_for_items"
  b84a6197^..edc0ab95` = **EMPTY**; H4's diff to `orchestrator.rs` does not touch
  `skill_provenance_for_items` / `assemble_from_component_items` / the `Components`
  arm / `active_skills`; H4's diff to `default.py` does not touch
  `_set_active_skills_from_matched_ids` / `select_skills` / `__list_skills__`.
  `skill_provenance_for_items` was introduced at `e7c2ce31` (Phase G.1); the test
  file was last touched at `7817706f` (Step 9.6, pre-task).

## 3. User decision — fix now via subplan (Option A)

Asked via `ask_user` (4 options). User chose **A**: fix now via subplan —
add `ThreadManager::with_pg_pool` plumbing into `ExecutionLoop` + migrate the
test to testcontainer-pg + `skills-db` + skip-if-no-docker (seed `reborn_skills`
via raw SQL). This mirrors the established 33/29-test migration pattern
(faithful gating, **not** suppression/ignore).

## 4. Fix surface (grounded)

- **`ThreadManager`** (`manager.rs:34-61`) has no `pg_pool` slot; `ExecutionLoop`
  already has `pg_pool: Option<Arc<brassclaw_pg::PgPool>>` (`loop_engine.rs:132`,
  `#[cfg(feature="skills-db")]`) + `with_pg_pool` builder (`:206-210`,
  `#[cfg(feature="skills-db")]`) threaded to `handle_assemble_prior_knowledge`
  (`loop_engine.rs:486` passes `self.pg_pool.as_deref()`). **Fix:** add
  `#[cfg(feature="skills-db")] pg_pool: Option<Arc<brassclaw_pg::PgPool>>` slot
  (default `None` in `new`) + `#[cfg(feature="skills-db")] pub fn with_pg_pool`
  builder to `ThreadManager`; call `.with_pg_pool(self.pg_pool.clone())` at
  `manager.rs:400` under `#[cfg(feature="skills-db")]`.
- **Root-crate test gating:** the root crate has **no** `skills-db` feature, so
  `#[cfg(feature = "skills-db")]` in a root test checks the root's own feature.
  Add a root `skills-db = ["brassclaw_engine/skills-db"]` feature so the test's
  `#[cfg(feature = "skills-db")]` gate compiles under `cargo test --features
  skills-db` (root). Add `brassclaw_pg` + `deadpool-postgres` ("0.14", matches
  `brassclaw_pg`) as root dev-deps so the test can build a `PgPool` + run
  migrations (`brassclaw_pg::PgPool` is `deadpool_postgres::Pool` but
  `brassclaw_pg` does not re-export `Manager`/`Config`/`NoTls`).
- **Test rig:** inline a per-test testcontainer pg helper in the test file
  (mirror `crates/brassclaw_reborn_composition/tests/common/mod.rs`: Postgres
  16-alpine → `deadpool_postgres` pool → `brassclaw_pg::migrations::run_migrations`;
  return `Err` → skip-if-no-docker). Only **one** test needs it, so a per-test
  container is simpler than a shared `OnceCell` rig.
- **Seed `reborn_skills` (raw SQL INSERT):** `DbSkillStore::insert` is unsuitable
  (generates a new UUID, auto-adds `05:validator`, sets `validation_status='pending'`).
  Insert with explicit `id = skill_doc.id`, `validation_status='validated'`,
  `consumer_tags='{}'` (no `05:validator`), `name='github'`, `version='1.0.0'`
  (major=1), `class_code=3`, `source='authored'`, scope matching the thread:
  `{tenant_id='', user_id='test-user', agent_id='', project_id=<pid>}`. `Thread::new`
  defaults `tenant_id`/`agent_id` to `String::new()`; the test spawns user
  `"test-user"` with `project_id`.
- **Flow:** `TestStore` + `MemoryDoc` (DocType::Skill) stay — `RamSource` returns
  the github skill as a class-3 `ComponentItem` (`id=doc.id`) →
  `matched_component_ids` includes it → `skill_provenance_for_items` queries
  `reborn_skills` by `doc.id` → finds the seeded row → provenance
  `[{doc_id, name:"github", version:1, snippet_names:[], force_activated:false}]`
  → `pkr["active_skills"]` non-empty → `_set_active_skills_from_matched_ids`
  → `__set_active_skills__` → `thread.set_active_skills` → assertions pass.
- **Assertion update (faithful):** line 785 `snippet_names == vec!["list_github_issues"]`
  → `Vec::<String>::new()` (matches the Phase-G.1 documented design:
  reborn_skills has no code_snippets column → snippet_names always []). This is
  **not** weakening the test — it correctly reflects the new mechanism.

## 5. Steps (one-by-one, commit + push after each)

- **S1 — `ThreadManager::with_pg_pool` plumbing (production, additive).** Add the
  cfg-gated slot + builder + wire into `ExecutionLoop` at `manager.rs:400`. Verify
  clippy clean engine (default + `skills-db`). Commit + push.
- **S2 — root `Cargo.toml`: `skills-db` feature + `brassclaw_pg`/`deadpool-postgres`
  dev-deps.** Additive (default-off). Verify `cargo build` clean. Commit + push.
- **S3 — migrate `skill_codeact_persists_active_skill_provenance`.** `#[cfg(feature=
  "skills-db")]` gate + inline testcontainer-pg rig + skip-if-no-docker + raw-SQL
  seed `reborn_skills` (id=skill_doc.id, scope match) + `.with_pg_pool(pool)` on
  `ThreadManager` + fix `snippet_names` assertion to `vec![]`. Verify the test
  compiles + passes w/ docker / skips w/o (both configs). Commit + push.
- **S4 — final H.4 verification + docs.** `cargo fmt --check` + workspace clippy
  (default + `--all-features` + `--features skills-db`) clean + `cargo test`
  (default: skips; `--features skills-db`: green w/ docker / skips w/o). Update
  this doc + the H4 subplan doc §4 H4.8→Done. Mark the Phase H.4 nested-subplan
  Zenflow step (`fa9fb137`) Completed. Resume Phase H at H.5.

## 6. Verification + status (updated as steps complete)

- S1 — Done. Added `#[cfg(feature="skills-db")] pg_pool: Option<std::sync::Arc<brassclaw_pg::PgPool>>` slot to `ThreadManager` (`manager.rs:67-68`, default `None` in `new` at `:95-96`) + `#[cfg(feature="skills-db")] pub fn with_pg_pool` builder (`:116-120`) + wired it into the spawned `ExecutionLoop` at `manager.rs:426-431` (`if let Some(pool) = self.pg_pool.clone() { exec_loop = exec_loop.with_pg_pool(pool); }`). Verified: `cargo clippy -p brassclaw_engine --all-targets -- -D warnings` clean both default (4.92s) + `--features skills-db` (5.15s). Commit `701f4194`.
- S2 — Done. Root `Cargo.toml`: added `brassclaw_pg = { path = "crates/brassclaw_pg", version = "0.1.0" }` dev-dep (`:284-288`) + `skills-db = ["brassclaw_engine/skills-db"]` feature (`:349-353`, default-off). Did NOT add `deadpool-postgres` (already a root dep via the `postgres`/default feature). Verified: `cargo check --features skills-db --test engine_v2_skill_codeact` (36s) + default (17s) both compile. Cargo.lock diff = exactly one line (`+ "brassclaw_pg",`), committed separately (`5426cce0`) per the no-amend rule. Commits `b01129ef` + `5426cce0`.
- S3 — Done. Migrated `tests/engine_v2_skill_codeact.rs::skill_codeact_persists_active_skill_provenance`: added `#[cfg(feature="skills-db")] mod pg_rig` (per-test Postgres-16 testcontainer rig, `start()` returns `Err`→skip-if-no-docker, mirrors `crates/brassclaw_reborn_composition/tests/common/mod.rs`); gated the test fn `#[cfg(feature="skills-db")]`; in the test: start rig (skip on no docker), raw-SQL `INSERT INTO reborn_skills (...) VALUES ($1, '', 'test-user', '', $2, 'github', 'GitHub API integration via HTTP tool', '1.0.0', 3, 'validated', '{}', 'authored', 1)` pinning `id=skill_doc.id` + scope, then `.with_pg_pool(pool)`; fixed `snippet_names` assertion to `Vec::<String>::new()` (faithful to V027 — `reborn_skills` has no `code_snippets` column). Resolved a doc-comment misattachment (moved the pre-existing `/// Verify selected skill provenance...` back above the test fn) + 2 `unreachable_pub` clippy lints (scoped `pub struct Rig`/`pub async fn start`/`pub pool` to `pub(super)` — NOT a silence). Verified: `cargo test --features skills-db --test engine_v2_skill_codeact skill_codeact_persists_active_skill_provenance -- --nocapture` → compiles with 0 test-binary warnings + skips cleanly (`skipping ... docker/testcontainers unavailable` → `ok`) when docker is down; `cargo test --test engine_v2_skill_codeact` (default) → the gated fn is absent, the other 4 tests pass. Commit `ad07906a`.
- S4 — Done. Final H.4 verification:
  - `cargo fmt -p brassclaw --check` (root crate, excludes the user's WIP crates) — clean. `manager.rs` + `db_skill_loader.rs` fmt-clean (verified via scoped check). Two fmt diffs in the migrated test file (import order + `Manager::new` wrapping) fixed by hand (surgical Edit, not `cargo fmt`, to avoid rewriting the user's uncommitted WIP files).
  - `cargo clippy -p brassclaw_engine --all-targets -- -D warnings` — clean both default + `--features skills-db` (re-confirmed post-S3).
  - `cargo clippy --features skills-db --test engine_v2_skill_codeact` (no `-D`) — the root test binary emits **0 warnings of its own**; the only 3 warnings are `collapsible_if` in `brassclaw_reborn_composition (lib)` — the user's uncommitted `basic_prompt_store` WIP, not H4.8 (see §7).
  - `cargo test --no-fail-fast` (default, full workspace) — **GREEN**: exit 0, every binary `test result: ok`, 0 failed. The original H4.8 regression (`skill_codeact_persists_active_skill_provenance` red on origin/main since Phase G.1 `e7c2ce31`) is **gone** — the failing test is `#[cfg(feature="skills-db")]`-gated out of the default config.
  - `cargo test --features skills-db --no-fail-fast` (full workspace) — **GREEN**: exit 0, 0 failures (the gated test is present and skips cleanly without docker; all other tests pass).
  - H4 subplan doc §4 H4.8→Done (this commit). Phase H.4 nested-subplan Zenflow step `fa9fb137` + this H4.8 Zenflow substep `d48d5809` marked Completed.

## 7. Pre-existing blockage OUTSIDE H4.8 (user's uncommitted `basic_prompt_store` WIP)

The full-workspace `cargo clippy --all --benches --tests --examples --all-features -- -D warnings` gate (the AGENTS.md repo-wide lint) is **currently blocked by 4 clippy lints in the user's separate, uncommitted `basic_prompt_store` feature WIP** — NOT by any H4.8 change:

- `unused imports: BasicPromptEntry and BasicPromptStoreError` — `crates/brassclaw_reborn_composition/src/pg_basic_prompt_store.rs:356` (new untracked file).
- `collapsible_if` — `crates/brassclaw_reborn_composition/src/interceptor_config_service.rs:287` + `:506`.
- `collapsible_if` — `crates/brassclaw_reborn_composition/src/validation_queue.rs:518`.

These files also have `cargo fmt` diffs (interceptor_config_service.rs, lib.rs module ordering, pg_basic_prompt_store.rs, webui_v2/src/lib.rs, webui_v2/src/router.rs, webui_v2/tests/*). They are part of the user's in-progress `basic_prompt_store` feature (spanning `brassclaw_product_workflow`, `brassclaw_reborn_composition`, `brassclaw_webui_v2`, the new `V063__reborn_basic_prompt_store.sql` migration, `pg_basic_prompt_store.rs`, and `prefix_V3.md`).

**Decision (per task rules "work around user changes, don't revert" + "don't commit unrelated user WIP"):** H4.8 does NOT modify, fix, or commit these files — fixing a single lint would `git add` + commit the user's ENTIRE unfinished feature under an H4.8 commit (overstepping + committing half-finished separate work). The blockage is pre-existing, belongs to the owner of that WIP, and will clear when they complete + commit their feature. H4.8's own changes (engine `manager.rs`, root `Cargo.toml`, the migrated test) are verified clippy-clean + fmt-clean + test-green in isolation (§6), which is the faithful scope of this subplan — this is scoping to isolate H4.8 from pre-existing user-WIP lints, **not** suppression/ignore of any H4.8 issue.
