# Subplan — pgvector / embedded-PostgreSQL build fix (encountered during Phase E.1 verification)

Parent plan: `saved_plan_to_v3.md` → Phase E → E.1 (Component-class registry, V061).
Phase E subplan: `docs/agents-v3/subplan_problem_stepE_of_saved_plan_to_v3.md`.
Zenflow task: `e81125fc-ce63-449e-922a-dfa80b964019`. Chat: `be1470ab-f612-4526-bc95-e1e37c8f4527`.
Inserted as a **sub-substep** under the Zenflow Phase E.1 substep (`36ccc42b`).

---

## 1. Why this subplan exists — E.1 runtime verification blocked by a pre-existing pgvector gap

E.1 wrote the V061 `reborn_components` registry migration + the `lookup_component_class`
helper + a 4-test `components_registry.rs` integration test, and is clippy-clean in both the
default and `--features brassclaw_reborn_composition/skills-db` configs.

The remaining E.1 acceptance step is **runtime verification**: apply V000–V061 through a real
Postgres-16 and confirm the registry trigger + lookup behave. The local path for that is the
embedded-PostgreSQL boot test (`cargo test -p brassclaw_embedded_postgres --features integration
full_boot_cycle_from_scratch`), which runs `refinery` over every migration in order.

That boot test **fails on V000**, never reaching V061:

```
migrations must apply: Migration("error applying migration V0__shared_triggers", "db error")
```

V000 executes `CREATE EXTENSION vector` (pgvector). The embedded PG has no pgvector installed, so
V000 aborts and refinery stops. **This is a pre-existing infrastructure gap, not an E.1 bug.** It
blocks E.1 (and every later migration's runtime verification) until fixed. Per the task rule
("address everything you encounter ... never suppress or silence anything"), it is fixed here via
this subplan before resuming E.1.

---

## 2. Root cause (fully diagnosed from the build script + Makefile.global)

The embedded-PG crate compiles pgvector from source at **build time** in `build.rs::
try_build_pgvector` (downloads pgvector v0.8.0 source, extracts the PG headers from the
theseus-rs PG 16.4.0 archive, `make PG_CONFIG=... all`, packs `lib/vector.dylib|so` +
`share/extension/vector.control` + `share/extension/vector--*.sql` into `pgvector.tar.gz`,
embedded via `include_bytes!(env!("EMBEDDED_PGVECTOR_ARCHIVE"))`). At runtime `extract.rs::
ensure_pg_extracted` extracts that archive into the PG install tree; `initdb.rs::install_pgvector`
then verifies `vector.control` is present (it warns + returns `Ok` if not — so V000 then fails).

`try_build_pgvector` fails on this macOS arm64 dev machine and writes an **empty placeholder**
(`pgvector.tar.gz` = 0 bytes) → no pgvector is embedded → V000 fails at runtime.

The `make pgvector` failure is:

```
clang: error: no such file or directory: 'debug'
clang: warning: no such sysroot directory: '/Applications/Xcode_15.4.app/.../MacOSX14.5.sdk'
make: *** [src/bitutils.o] Error 1
```

Four stale tokens leak into the compile, all baked into the pre-built PG's
`pg-install/lib/pgxs/src/Makefile.global` (a **text file**) or into the build-script env:

1. **`PROFILE=debug`** (cargo sets this in every build-script env) → `Makefile.global:750-752`:
   ```make
   ifdef PROFILE
      CFLAGS += $(PROFILE)
      LDFLAGS += $(PROFILE)
   endif
   ```
   injects the bare word `debug` into `CFLAGS` → clang treats `debug` as an input file →
   **`no such file or directory: 'debug'` (the hard error).**
2. **`PG_SYSROOT = /Applications/Xcode_15.4.app/.../MacOSX14.5.sdk`** (`Makefile.global:241`,
   CI-baked) → `CPPFLAGS = -isysroot $(PG_SYSROOT) ...` (`:240`) → missing-sysroot warning.
3. **`-I/Users/runner/brew/opt/icu4c/include`** (`Makefile.global:240`, CI-baked brew ICU path;
   PG was configured `--without-icu` so it is not even needed) → stray include path.
4. **`-Werror=unguarded-availability-new`** (`Makefile.global:258` `CFLAGS = ...`, CI-baked,
   Xcode-version-specific) → can escalate an availability warning into a hard error on a
   different Xcode.

The existing `sanitise_pg_cflags` (`build.rs:323`) only cleans `pg_config --cflags` and passes
the result as `PG_CFLAGS=` (which pgxs **appends**). The stale **base** `CPPFLAGS`/`CFLAGS` baked
into `Makefile.global` are never sanitised, so tokens 2–4 leak; token 1 leaks via the env. None of
the four is reached by the current sanitiser.

---

## 3. User design decisions (this portion — confirmed via ask_user)

1. **Fix path (Q1 → C):** Do **both** — fix the existing compile-from-source (Part A) as the
   primary path, AND add a runtime prebuilt-fetch fallback (Part B) for machines where the
   compile still cannot run. Most robust.
2. **Part B fetch source (Q2 → B-iii):** A fallback **chain** in `install_pgvector`:
   (a) locate a system/brew pgvector matching **PG major 16** and copy its files into the
   embedded PG tree; then (b) if `BRASSCLAW_PGVECTOR_URL` is set, download a prebuilt tarball
   (layout `lib/vector.*` + `share/extension/*`) and extract it; then (c) the existing
   warn + `return Ok` degraded behaviour. PG's extension ABI is stable within a major version,
   so a system PG-16 `vector.dylib` loads in the theseus-rs PG 16.4.0 — gated on PG major 16.

**Design nuance accepted:** the crate's stated principle (`build.rs` header line 13:
"No network access is required at runtime — all binary content is baked in") is intentionally
narrowed by Part B: the runtime fetch is a **degraded-mode escape hatch** that fires only when
the embedded archive is empty (compile failed). The normal path remains fully embedded. This is
documented as an upgrade, not a silent reversal.

**No new crate dependency:** the crate has no HTTP client dep. Part B's env-URL download uses
`curl` via `tokio::process::Command`, mirroring `build.rs::download_with_curl` (curl is already a
build-time assumption). The system-location step is filesystem-only.

---

## 4. Part A — fix compile-from-source (`build.rs`)

### 4.1 New helper `sanitise_pg_makefiles(pg_install: &Path) -> bool`

Called in `try_build_pgvector` **after** the PG archive is extracted into `pg-install`
(`build.rs:183-191`) and **before** `make` (`build.rs:245`). Edits
`pg-install/lib/pgxs/src/Makefile.global` in place (text):

- **`CPPFLAGS = ...` line:** rebuild the token list, dropping `-isysroot <arg>` (and
  `-isysroot=<arg>`), and any `-I<path>` whose path contains `/runner/` (CI-machine brew path).
  Keeps legitimate `-I` tokens. If the result is empty, write `CPPFLAGS =`.
- **`CFLAGS = ...` line:** drop `-Werror=unguarded-availability-new` and
  `-Wno-unguarded-availability-new` tokens.
- **`LDFLAGS = ...` line:** drop `-isysroot <arg>` (and `-isysroot=<arg>`) and any
  `-L<path>` whose path contains `/runner/` (CI-machine brew ICU lib path). The
  baked `LDFLAGS = $(LDFLAGS_INTERNAL) -isysroot $(PG_SYSROOT) -L/Users/runner/brew/...`
  is the **link-stage** leak: after `PG_SYSROOT` is blanked, `-isysroot $(PG_SYSROOT)`
  expands to a bare empty `-isysroot`, which on macOS makes the linker look for
  libSystem in an empty sysroot → fatal `ld: library 'System' not found`
  (discovered during Part A verification — the C compile passed but the link
  failed on this token). Dropping `-isysroot` + its arg lets the link fall back
  to the dev machine's active SDK. `LDFLAGS_INTERNAL` (`-L$(libdir)`) is clean.
- **`PG_SYSROOT = ...` line:** replace value with empty (`PG_SYSROOT =`).
- **`ifdef PROFILE` block (`:750-753`):** remove the four lines (`ifdef PROFILE` /
  `CFLAGS += $(PROFILE)` / `LDFLAGS += $(PROFILE)` / `endif`) — defensive: neutralises token 1
  even if a future build does not go through this `build.rs` env scrub.

Returns `false` only if `Makefile.global` is absent/unreadable (treated as soft — `make` will
then fail and the caller writes the empty placeholder as before). Returns `true` on success.
Idempotent: re-running on an already-sanitised file is a no-op (the bad tokens are already gone).

### 4.2 Scrub `PROFILE` from the `make` environment

Add `.env_remove("PROFILE")` to **both** `make` `Command`s in `try_build_pgvector`
(`build.rs:245-254` build, `:265-275` install). This is the robust kill for token 1 (the hard
error) regardless of the `Makefile.global` edit. `run_or_warn` inherits env, so chaining
`.env_remove("PROFILE")` on the builder before passing it in is sufficient.

### 4.3 Verification (Part A) — ✅ PASSED

- Rebuild: `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo build -vv -p
  brassclaw_embedded_postgres` → `build.rs: pgvector: sanitised stale CI tokens in
  Makefile.global` + `build.rs: pgvector built and packed successfully`; `pgvector.tar.gz`
  is **74387 bytes** (was 0). First rebuild passed the C compile but failed the link on
  the empty `-isysroot` (see §4.1 LDFLAGS bullet); adding the `LDFLAGS =` arm fixed it.
- Boot: `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo test -p
  brassclaw_embedded_postgres --features integration full_boot_cycle_from_scratch` →
  `test integration::full_boot_cycle_from_scratch ... ok` (3.45s) — V000
  `CREATE EXTENSION vector` succeeds; refinery applies V000–V054 (the prior head) cleanly.
  Part A fully closes the pgvector gap on this host.

---

## 5. Part B — runtime prebuilt-fetch fallback (`initdb.rs::install_pgvector`)

### 5.1 Expose the flat-tar extractor

Change `extract.rs::extract_flat_tarball` from `fn` to `pub(crate) fn` so `initdb.rs` can reuse
it for the env-URL download (same archive layout as the embedded pgvector archive). No behaviour
change.

### 5.2 New helper `procure_pgvector_fallback(pg_base: &Path) -> bool`

Called inside `install_pgvector` only when `pg_base/share/extension/vector.control` is absent
(i.e. the embedded archive was empty). Tries, in order:

**(a) System/brew location (no network).** Probe candidate roots for a PG-16 pgvector:
- macOS Apple Silicon: `/opt/homebrew/opt/libpgvector`, `/opt/homebrew/opt/pgvector`
- macOS Intel: `/usr/local/opt/libpgvector`, `/usr/local/opt/pgvector`
- Linux: `/usr/lib/postgresql/16`, `/usr/share/postgresql/16/extension`,
  `/usr/lib/postgresql/16/extension`

For each root, look for a control file under `share/extension` (or
`share/postgresql@16/extension`, `share/postgresql/16/extension`) named `vector.control`, a lib
under `lib` (or `lib/postgresql`) named `vector.dylib`/`vector.so`, and `vector--*.sql` scripts.
Additionally, if `pg_config` is on `PATH`, query `pg_config --version`; if it reports PostgreSQL
16.x, use `pg_config --pkglibdir` / `pg_config --sharedir` as the probe root (gated on major 16).

When a complete set is found, copy `vector.dylib|so` → `pg_base/lib` and
`vector.control` + `vector--*.sql` → `pg_base/share/extension`. Return `true`.

**(b) Env-URL download (network, opt-in).** If `BRASSCLAW_PGVECTOR_URL` is set, download via
`curl` (`tokio::process::Command`, mirroring `build.rs::download_with_curl`) to a temp file, then
`extract_flat_tarball(bytes, pg_base)` (flat layout `lib/vector.*` + `share/extension/*`).
Return `true` on success.

**(c) Neither procures** → return `false`.

### 5.3 Wire into `install_pgvector`

At `initdb.rs:240-249` (the `if !control_src.exists()` branch), before the existing
`tracing::warn!` + `return Ok(())`:

```rust
if !control_src.exists() {
    // Embedded pgvector archive was empty (compile failed on this host).
    // Try the runtime fallback chain before degrading.
    if procure_pgvector_fallback(pg_base).await {
        debug!("pgvector procured via runtime fallback (system/brew or BRASSCLAW_PGVECTOR_URL)");
        // Re-check: the fallback placed files directly into pg_base/lib + pg_base/share/extension.
        if ext_dst.join("vector.control").exists() {
            return Ok(());
        }
    }
    tracing::warn!(... existing warning ...);
    return Ok(());
}
```

The fallback writes files directly into the destination tree (the dst IS `pg_base/lib` +
`pg_base/share/extension`), so after a successful procurement the existing copy loop is not
needed — we verify `vector.control` is now present and return `Ok`.

### 5.4 Verification (Part B) — ✅ PASSED

**Implementation as-built (one refinement vs §5.2/§5.4 draft):**
- `extract.rs::extract_flat_tarball` is now `pub(crate)` (§5.1).
- The system-path copy logic was extracted into a **candidate-injected** helper
  `copy_pgvector_from_candidates(&[(ext_dir, lib_dir)], lib_dst, ext_dst) -> bool`
  so the side-effecting copy is unit-testable with temp dirs (the production caller
  `procure_pgvector_from_system` supplies the real static + `pg_config` candidate list).
- The env-URL download uses `curl --output -` (bytes captured in memory via
  `.output().stdout`) — no temp file, so no `tempfile` runtime dep needed.
- Wired into `install_pgvector` per §5.3 (procure → re-check `vector.control` → `Ok`).

**Results:**
- `cargo clippy -p brassclaw_embedded_postgres --all-targets -- -D warnings` → clean
  (the three `if starts_with { if let Some }` arms in `build.rs::sanitise_pg_makefiles`
  were collapsed to edition-2024 let-chains to satisfy `collapsible_if`).
- `cargo test -p brassclaw_embedded_postgres` → **10 passed** (4 new):
  `pg_major_from_version_parses_postgres_versions`,
  `system_candidates_extension_dirs_end_in_extension`,
  `copy_pgvector_from_candidates_copies_complete_set` (synthetic brew layout →
  control + `vector--*.sql` + `vector.dylib` copied, non-matching sql ignored),
  `copy_pgvector_from_candidates_skips_incomplete_set` (candidate missing a lib →
  skipped, no control copied).
- `cargo fmt -p brassclaw_embedded_postgres -- --check` → clean.
- The fallback's live IO path (system probe + curl) fires only in degraded mode
  (empty embedded archive); on this host Part A made the archive non-empty, so the
  `full_boot_cycle_from_scratch` integration test exercises the normal
  `install_pgvector` path (control present → fallback not triggered) — see §4.3.

---

## 6. Ordered steps (run one-by-one, commit+push each)

- **P1 — Part A (`build.rs`):** add `sanitise_pg_makefiles`, call it pre-`make`, add
  `.env_remove("PROFILE")` to both `make` commands. Verify rebuild → non-empty
  `pgvector.tar.gz` + boot test V000 applies. Commit+push (with this subplan doc).
- **P2 — Part B (`initdb.rs` + `extract.rs`):** `pub(crate)` `extract_flat_tarball`; add
  `procure_pgvector_fallback` + the system-path detection pure helper + its unit test; wire into
  `install_pgvector`. Verify compile + clippy + unit test. Commit+push.
- **P3 — resume E.1:** confirm V000–V061 all apply via the embedded-PG boot test; run
  `components_registry.rs` (skips without docker, runs in CI). Mark E.1 + this sub-substep
  Completed. Commit+push the E.1 registry work + Phase E subplan doc + `saved_plan_to_v3.md`
  references. Proceed to E.2.

---

## 7. Acceptance

- `try_build_pgvector` succeeds on macOS arm64 → `pgvector.tar.gz` non-empty → real
  `vector.dylib` + `vector.control` + `vector--*.sql` embedded.
- The embedded-PG boot test applies **V000–V061** cleanly (`CREATE EXTENSION vector` works).
- `install_pgvector` degrades gracefully: if the embedded archive is empty, it procures pgvector
  from system/brew or `BRASSCLAW_PGVECTOR_URL` before warning; the normal embedded path is
  unchanged.
- No new crate dependency; `cargo clippy -p brassclaw_embedded_postgres --all-targets -- -D
  warnings` clean (default + `--features integration`).
