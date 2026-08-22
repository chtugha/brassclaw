/// Build script for brassclaw_embedded_postgres.
///
/// Downloads and verifies the PostgreSQL 16.4.0 binary archive for the current
/// compile target at build time, emitting `EMBEDDED_PG_ARCHIVE` so that
/// `include_bytes!` bakes the bytes into the binary.
///
/// For Linux and macOS targets this script also downloads the pgvector source,
/// compiles it against the PG headers from the downloaded archive, and produces
/// the extension files (`vector.control`, `vector--*.sql`, `vector.so/dylib`).
/// These are packed into a second archive (`EMBEDDED_PGVECTOR_ARCHIVE`) that is
/// also embedded and extracted alongside the PG binaries on first boot.
///
/// No network access is required at runtime — all binary content is baked in.
use std::path::{Path, PathBuf};
use std::process::Command;

// ── version constants ─────────────────────────────────────────────────────────
const PG_VERSION: &str = "16.4.0";
const PG_BASE_URL: &str = "https://github.com/theseus-rs/postgresql-binaries/releases/download";
const PGVECTOR_VERSION: &str = "0.8.0";
const PGVECTOR_URL: &str = "https://github.com/pgvector/pgvector/archive/refs/tags/v0.8.0.tar.gz";

// ── per-platform checksums (must stay in sync with checksums.rs) ──────────────
fn expected_pg_sha256(target: &str) -> Option<&'static str> {
    match target {
        "x86_64-unknown-linux-gnu" => {
            Some("1059350056c24e6dd3974af7582199c2a4d06078ecb2beb9f4b26b6debea6d37")
        }
        "x86_64-unknown-linux-musl" => {
            Some("7cc4ba47dd1ceb61876b59094ed7e09bf118e903ef23dbc8ff453f4a3d782f17")
        }
        "aarch64-unknown-linux-gnu" => {
            Some("62736e25b44c92ca8621987df9a8940ccbad158ad30dcbda105b4587fa5db2b3")
        }
        "aarch64-unknown-linux-musl" => {
            Some("9013ef13b7677693ecf69cf09d6e4e64789809b910c02517deb118e995cac2e1")
        }
        "aarch64-apple-darwin" => {
            Some("0ec91e77eff381e43e3963f012aff3acb9de12ad3739a625e57cce9671b28b0f")
        }
        "x86_64-apple-darwin" => {
            Some("3193b9747c610139990c9913ff5fd5ad73cd38cefcd5ffcdc46079fd1479406e")
        }
        "x86_64-pc-windows-msvc" => {
            Some("e01be1f09d72f989f998845765786c779f78128eae0edb99380285838d34c447")
        }
        _ => None,
    }
}

/// Returns true if this target needs pgvector compiled from source.
fn needs_pgvector_compile(target: &str) -> bool {
    target.contains("linux") || target.contains("apple-darwin")
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=src/checksums.rs");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
    let target = std::env::var("TARGET").expect("TARGET not set");
    let pg_archive = out_dir.join("postgresql.tar.gz");

    // ── Step 1: PostgreSQL archive ────────────────────────────────────────────
    if expected_pg_sha256(&target).is_none() {
        // Unsupported target: write empty placeholder so include_bytes! compiles.
        if !pg_archive.exists() {
            std::fs::write(&pg_archive, b"").expect("write placeholder");
        }
        let pgvector_archive = out_dir.join("pgvector.tar.gz");
        if !pgvector_archive.exists() {
            std::fs::write(&pgvector_archive, b"").expect("write placeholder");
        }
        println!(
            "cargo:rustc-env=EMBEDDED_PG_ARCHIVE={}",
            pg_archive.display()
        );
        println!(
            "cargo:rustc-env=EMBEDDED_PGVECTOR_ARCHIVE={}",
            pgvector_archive.display()
        );
        eprintln!("build.rs: no pre-built archive for {target}; skipping");
        return;
    }

    if !pg_archive.exists() || !verify_sha256(&pg_archive, &target, expected_pg_sha256) {
        let archive_name = format!("postgresql-{PG_VERSION}-{target}.tar.gz");
        let url = format!("{PG_BASE_URL}/{PG_VERSION}/{archive_name}");
        eprintln!("build.rs: downloading PostgreSQL {PG_VERSION} for {target}");
        download_with_curl(&url, &pg_archive);
        assert!(
            verify_sha256(&pg_archive, &target, expected_pg_sha256),
            "SHA-256 mismatch for PostgreSQL archive (target={target})"
        );
    } else {
        eprintln!(
            "build.rs: PostgreSQL archive cached at {}",
            pg_archive.display()
        );
    }
    println!(
        "cargo:rustc-env=EMBEDDED_PG_ARCHIVE={}",
        pg_archive.display()
    );

    // ── Step 2: pgvector extension files ─────────────────────────────────────
    let pgvector_archive = out_dir.join("pgvector.tar.gz");

    if needs_pgvector_compile(&target) {
        if !pgvector_archive.exists()
            || pgvector_archive.metadata().map(|m| m.len()).unwrap_or(0) < 1024
        {
            if !try_build_pgvector(&out_dir, &pg_archive, &pgvector_archive, &target) {
                // Build failed (e.g. missing SDK on macOS dev machine). Write
                // an empty placeholder so `include_bytes!` compiles; pgvector
                // will not be embedded and `install_pgvector` will emit a
                // warning at runtime instead of failing hard.
                eprintln!(
                    "build.rs: pgvector build failed for {target}; \
                     embedding empty placeholder (CREATE EXTENSION vector will \
                     warn at runtime unless pgvector is available globally)"
                );
                if !pgvector_archive.exists() {
                    std::fs::write(&pgvector_archive, b"")
                        .expect("write empty pgvector placeholder");
                }
            }
        } else {
            eprintln!(
                "build.rs: pgvector archive cached at {}",
                pgvector_archive.display()
            );
        }
    } else {
        // Windows: write empty placeholder; pgvector is not needed on Windows
        // because the V000 migration is not expected to run there in CI.
        if !pgvector_archive.exists() {
            std::fs::write(&pgvector_archive, b"").expect("write placeholder");
        }
    }
    println!(
        "cargo:rustc-env=EMBEDDED_PGVECTOR_ARCHIVE={}",
        pgvector_archive.display()
    );
}

/// Download pgvector source, extract the PG headers from the PG archive, compile
/// the pgvector extension, and pack the output files into `pgvector_archive`.
///
/// Returns `true` on success, `false` if any step fails (e.g. missing SDK on
/// a macOS dev machine where the pre-built PG binary encodes a stale sysroot).
/// On failure the caller should write an empty placeholder so `include_bytes!`
/// still compiles; `install_pgvector()` in `initdb.rs` handles the missing
/// extension gracefully with a runtime warning.
///
/// The packed archive contains:
///   `lib/vector.so` (or `.dylib` on macOS)
///   `share/extension/vector.control`
///   `share/extension/vector--*.sql`
///
/// These paths match the layout that `install_pgvector()` in `initdb.rs` looks
/// for relative to the installation root.
fn try_build_pgvector(
    out_dir: &Path,
    pg_archive: &Path,
    pgvector_archive: &Path,
    target: &str,
) -> bool {
    let build_dir = out_dir.join("pgvector-build");
    if let Err(e) = std::fs::create_dir_all(&build_dir) {
        eprintln!("build.rs: pgvector: create build dir failed: {e}");
        return false;
    }

    // 1. Extract the PG installation into build_dir/pg-install so we have pg_config.
    let pg_install = build_dir.join("pg-install");
    if let Err(e) = std::fs::create_dir_all(&pg_install) {
        eprintln!("build.rs: pgvector: create pg-install dir failed: {e}");
        return false;
    }
    eprintln!("build.rs: extracting PG headers for pgvector build");
    if !run_or_warn(
        Command::new("tar")
            .args(["xzf", &pg_archive.display().to_string()])
            .args(["--strip-components=1"])
            .args(["-C", &pg_install.display().to_string()]),
        "extract PG archive for pgvector build",
    ) {
        return false;
    }

    // 1b. Sanitise the pre-built PG's Makefile.global. The theseus-rs binaries
    // bake the CI machine's toolchain into the base CPPFLAGS/CFLAGS:
    //   - `PG_SYSROOT = /Applications/Xcode_15.4.app/.../MacOSX14.5.sdk` (absent
    //     on dev machines) → CPPFLAGS `-isysroot $(PG_SYSROOT)`.
    //   - `-I/Users/runner/brew/opt/icu4c/include` (CI brew ICU; PG was built
    //     --without-icu so it is not even needed).
    //   - `-Werror=unguarded-availability-new` (Xcode-version-specific).
    // and an `ifdef PROFILE / CFLAGS += $(PROFILE)` block that injects cargo's
    // build-script `PROFILE=debug` env as a bare `debug` token → clang aborts
    // with "no such file or directory: 'debug'". These leak via the base
    // CPPFLAGS/CFLAGS, which the PG_CFLAGS sanitiser below does NOT reach (it
    // only cleans the appended PG_CFLAGS). Strip them here so `make` succeeds.
    sanitise_pg_makefiles(&pg_install);

    // 2. Download pgvector source.
    let pgvector_src_archive = build_dir.join("pgvector-src.tar.gz");
    if !pgvector_src_archive.exists() {
        eprintln!("build.rs: downloading pgvector {PGVECTOR_VERSION} source");
        download_with_curl(PGVECTOR_URL, &pgvector_src_archive);
    }
    // Extract.
    let pgvector_src = build_dir.join(format!("pgvector-{PGVECTOR_VERSION}"));
    if !pgvector_src.exists()
        && !run_or_warn(
            Command::new("tar")
                .args(["xzf", &pgvector_src_archive.display().to_string()])
                .args(["-C", &build_dir.display().to_string()]),
            "extract pgvector source",
        )
    {
        return false;
    }

    // 3. Build: make PG_CONFIG=<path>.
    //
    // The primary detox of the pre-built PG's stale CI toolchain is done in
    // `sanitise_pg_makefiles` above (Makefile.global base CPPFLAGS/CFLAGS +
    // the `ifdef PROFILE` block). Two further guards here:
    //   - `.env_remove("PROFILE")`: cargo sets PROFILE=debug in every
    //     build-script env; even with the Makefile.global block removed, this
    //     prevents any residual `ifdef PROFILE` path from injecting `debug`.
    //   - `PG_CFLAGS=<sanitised>`: a sanitised copy of `pg_config --cflags`
    //     (stripped of -isysroot / bare words / -W*-availability-new) is
    //     appended by pgxs, belt-and-suspenders against future stale tokens.
    // Compiling an extension against headers only does not need the full PG
    // server sysroot on the same OS family.
    let pg_config = pg_install.join("bin").join("pg_config");
    let pg_config_str = pg_config.display().to_string();

    // Probe pg_config --cflags to detect a stale sysroot before wasting time.
    let cflags_output = Command::new(&pg_config)
        .arg("--cflags")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();

    // Build a sanitised CFLAGS: strip -isysroot + its path argument, strip
    // bare word tokens that are not flags (no leading -), and strip stale
    // macOS-version-specific availability flags.
    let sanitised_cflags = sanitise_pg_cflags(&cflags_output);

    eprintln!("build.rs: compiling pgvector with pg_config={pg_config_str}");
    eprintln!("build.rs: using sanitised CFLAGS: {sanitised_cflags}");

    if !run_or_warn(
        Command::new("make")
            .args([
                &format!("PG_CONFIG={pg_config_str}"),
                &format!("PG_CFLAGS={sanitised_cflags}"),
                "all",
            ])
            .current_dir(&pgvector_src)
            .env_remove("PROFILE"),
        "make pgvector",
    ) {
        return false;
    }

    // 4. Install into a staging directory.
    let staging = build_dir.join("pgvector-staging");
    if let Err(e) = std::fs::create_dir_all(&staging) {
        eprintln!("build.rs: pgvector: create staging dir failed: {e}");
        return false;
    }
    let staging_str = staging.display().to_string();
    if !run_or_warn(
        Command::new("make")
            .args([
                &format!("PG_CONFIG={pg_config_str}"),
                &format!("DESTDIR={staging_str}"),
                "install",
            ])
            .current_dir(&pgvector_src)
            .env_remove("PROFILE"),
        "make install pgvector",
    ) {
        return false;
    }

    // 5. Find the installed files and compute their paths relative to the PG
    //    installation root (as returned by pg_config --pkglibdir /
    //    pg_config --sharedir). We want paths relative to the installation root
    //    so that extract.rs can place them in the correct location.
    //
    // The staging dir layout after `make install DESTDIR=...` is:
    //   staging/{pg_config --pkglibdir}/vector.so     (or vector.dylib on macOS)
    //   staging/{pg_config --sharedir}/extension/vector.control
    //   staging/{pg_config --sharedir}/extension/vector--*.sql
    //
    // pg_config --pkglibdir typically returns e.g. /usr/lib/postgresql/16/lib
    // We find the files by walking the staging tree and matching names.

    // Collect all files to pack.
    let files = collect_pgvector_files(&staging);
    if files.is_empty() {
        eprintln!(
            "build.rs: pgvector build produced no output files under {staging_str}; \
             check the make output above"
        );
        return false;
    }

    // 6. Pack the files into pgvector.tar.gz using relative paths that match
    //    the installation root layout expected by install_pgvector().
    //    We normalise the paths: anything under .../lib/ becomes lib/<name>,
    //    anything under .../extension/ becomes share/extension/<name>.
    eprintln!(
        "build.rs: packing pgvector extension files into {}",
        pgvector_archive.display()
    );
    pack_pgvector_files(&files, &staging, pgvector_archive, target);
    eprintln!("build.rs: pgvector built and packed successfully");
    true
}

/// Sanitise the pre-built PostgreSQL `Makefile.global` so pgvector compiles
/// on a dev machine whose toolchain differs from the CI machine that produced
/// the theseus-rs binaries.
///
/// Edits `<pg_install>/lib/pgxs/src/Makefile.global` in place:
/// - `CPPFLAGS =` assignment: drop `-isysroot <arg>` (and `-isysroot=<arg>`)
///   and any `-I<path>` whose path contains `/runner/` (the CI brew ICU path;
///   PG was built `--without-icu` so it is unused).
/// - `CFLAGS =` assignment: drop `-Werror=unguarded-availability-new` and
///   `-Wno-unguarded-availability-new` (Xcode-version-specific).
/// - `LDFLAGS =` assignment: drop `-isysroot <arg>` (and `-isysroot=<arg>`)
///   and any `-L<path>` whose path contains `/runner/` (the CI brew ICU lib
///   path; PG was built `--without-icu` so it is unused). The baked
///   `LDFLAGS = $(LDFLAGS_INTERNAL) -isysroot $(PG_SYSROOT) -L/Users/runner/...
///   line is the link-stage leak: once `PG_SYSROOT` is blanked, `-isysroot
///   $(PG_SYSROOT)` expands to a bare empty `-isysroot`, which on macOS makes
///   the linker look for libSystem in an empty sysroot → fatal
///   `ld: library 'System' not found`. Dropping `-isysroot` + its arg lets the
///   link fall back to the dev machine's active SDK. `LDFLAGS_INTERNAL`
///   (`-L$(libdir)`) is clean and left untouched.
/// - `PG_SYSROOT =` assignment: blank the value (no longer referenced after
///   the CPPFLAGS/LDFLAGS edits, but kept clean).
/// - `ifdef PROFILE ... endif` block: removed entirely. cargo sets
///   `PROFILE=debug` in build-script envs, and `CFLAGS += $(PROFILE)` injects
///   a bare `debug` token that clang treats as an input file → fatal
///   "no such file or directory: 'debug'". (Also neutralised at the `make`
///   call site via `.env_remove("PROFILE")`.)
///
/// Only the bare `NAME =` assignments are rewritten — `override` / `+=` /
/// `NAME_SL` lines are left untouched. Idempotent: re-running on an already
/// sanitised file changes nothing.
///
/// Returns `true` on success (file read + written, or no change needed),
/// `false` only if `Makefile.global` is absent/unreadable (treated as soft —
/// `make` will then fail and the caller writes the empty placeholder).
fn sanitise_pg_makefiles(pg_install: &Path) -> bool {
    let path = pg_install
        .join("lib")
        .join("pgxs")
        .join("src")
        .join("Makefile.global");
    let Ok(content) = std::fs::read_to_string(&path) else {
        eprintln!(
            "build.rs: pgvector: Makefile.global not found at {} (skipping sanitise)",
            path.display()
        );
        return false;
    };

    let mut out_lines: Vec<String> = Vec::with_capacity(content.lines().count());
    let mut changed = false;
    // >0 while skipping an `ifdef PROFILE ... endif` block slated for removal.
    let mut profile_block_depth: i32 = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        // Inside an ifdef PROFILE block being removed: skip until the matching
        // endif, tracking nested conditionals so an inner ifdef doesn't fool us.
        if profile_block_depth > 0 {
            if trimmed.starts_with("ifdef")
                || trimmed.starts_with("ifndef")
                || trimmed.starts_with("ifeq")
                || trimmed.starts_with("ifneq")
            {
                profile_block_depth += 1;
            } else if trimmed == "endif" {
                profile_block_depth -= 1;
            }
            changed = true;
            continue;
        }

        // Start of the block we remove. Match `ifdef PROFILE` exactly (not
        // `ifdef COPT` or other conditionals).
        if trimmed == "ifdef PROFILE" {
            profile_block_depth = 1;
            changed = true;
            continue;
        }

        // `CPPFLAGS = ...` (the bare assignment, not `override ...` / `:=`).
        if (line.starts_with("CPPFLAGS =") || line.starts_with("CPPFLAGS="))
            && let Some(new) = sanitise_flags_assignment(line, |tok| {
                if tok.starts_with("-isysroot=") {
                    return true;
                }
                if let Some(rest) = tok.strip_prefix("-I") {
                    // Drop CI-machine brew include paths (PG is --without-icu).
                    if rest.contains("/runner/") {
                        return true;
                    }
                }
                false
            })
        {
            out_lines.push(new);
            changed = true;
            continue;
        }

        // `CFLAGS = ...` (the bare assignment).
        if (line.starts_with("CFLAGS =") || line.starts_with("CFLAGS="))
            && let Some(new) = sanitise_flags_assignment(line, |tok| {
                tok == "-Werror=unguarded-availability-new"
                    || tok == "-Wno-unguarded-availability-new"
            })
        {
            out_lines.push(new);
            changed = true;
            continue;
        }

        // `LDFLAGS = ...` (the bare assignment). The theseus-rs Makefile.global
        // bakes `LDFLAGS = $(LDFLAGS_INTERNAL) -isysroot $(PG_SYSROOT)
        // -L/Users/runner/brew/opt/icu4c/lib -Wl,-dead_strip_dylibs`. Two leaks:
        //   - `-isysroot $(PG_SYSROOT)`: PG_SYSROOT was blanked above, so this
        //     expands to a bare `-isysroot` with an EMPTY argument. On macOS that
        //     makes the linker look for libSystem in an empty sysroot → fatal
        //     `ld: library 'System' not found`. Drop `-isysroot` + its arg token
        //     (here the `$(PG_SYSROOT)` make var ref) so the link falls back to
        //     the dev machine's active SDK (the default without `-isysroot`).
        //   - `-L/Users/runner/brew/opt/icu4c/lib`: CI-machine brew ICU lib path;
        //     PG was built `--without-icu` so it is unused.
        // `LDFLAGS_INTERNAL` is clean (`-L$(libdir)`) so it is left untouched.
        if (line.starts_with("LDFLAGS =") || line.starts_with("LDFLAGS="))
            && let Some(new) = sanitise_flags_assignment(line, |tok| {
                if tok.starts_with("-isysroot=") {
                    return true;
                }
                if let Some(rest) = tok.strip_prefix("-L") {
                    // Drop CI-machine brew library paths (PG is --without-icu).
                    if rest.contains("/runner/") {
                        return true;
                    }
                }
                false
            })
        {
            out_lines.push(new);
            changed = true;
            continue;
        }

        // `PG_SYSROOT = <stale CI SDK path>` → blank.
        if line.starts_with("PG_SYSROOT =") || line.starts_with("PG_SYSROOT=") {
            let blanked = "PG_SYSROOT =".to_string();
            if blanked != line {
                out_lines.push(blanked);
                changed = true;
                continue;
            }
        }

        out_lines.push(line.to_string());
    }

    if !changed {
        eprintln!("build.rs: pgvector: Makefile.global already sanitised (no change)");
        return true;
    }

    let new_content = format!("{}\n", out_lines.join("\n"));
    if let Err(e) = std::fs::write(&path, new_content) {
        eprintln!(
            "build.rs: pgvector: failed to write sanitised Makefile.global ({}): {e}",
            path.display()
        );
        return false;
    }
    eprintln!("build.rs: pgvector: sanitised stale CI tokens in Makefile.global");
    true
}

/// Rebuild a `NAME = tokens` assignment line, dropping tokens for which
/// `drop_token(tok)` returns true, and (always) dropping `-isysroot` plus the
/// path token that follows it. Returns `Some(new_line)` when the value
/// changed, `None` when it is unchanged (so the caller can leave the line as
/// is). The rebuilt line preserves the `NAME =` prefix and rejoins the kept
/// tokens with single spaces.
fn sanitise_flags_assignment(line: &str, drop_token: impl Fn(&str) -> bool) -> Option<String> {
    let eq = line.find('=')?;
    let name = line[..eq].trim_end();
    let value = line[eq + 1..].trim();
    let tokens: Vec<&str> = value.split_whitespace().collect();

    let mut kept: Vec<&str> = Vec::with_capacity(tokens.len());
    let mut skip_next = false;
    for tok in &tokens {
        if skip_next {
            skip_next = false;
            continue;
        }
        if *tok == "-isysroot" {
            skip_next = true;
            continue;
        }
        if drop_token(tok) {
            continue;
        }
        kept.push(tok);
    }

    let new_value = kept.join(" ");
    if new_value == value {
        return None;
    }
    Some(if new_value.is_empty() {
        format!("{name} =")
    } else {
        format!("{name} = {new_value}")
    })
}

/// Sanitise a `pg_config --cflags` string for cross-SDK use.
///
/// Removes tokens that are valid only on the SDK that compiled the pre-built
/// PG binary but may be absent on the developer's machine:
///
/// - `-isysroot <path>` (the following path token is consumed too)
/// - Bare words that are not flags (no leading `-`), e.g. the stray `debug`
///   token that some pre-built PG binaries inject as a positional argument.
/// - `-Werror=unguarded-availability-new` (Xcode-version-specific)
fn sanitise_pg_cflags(raw: &str) -> String {
    let tokens: Vec<&str> = raw.split_whitespace().collect();
    let mut out = Vec::with_capacity(tokens.len());
    let mut skip_next = false;

    for token in &tokens {
        if skip_next {
            skip_next = false;
            continue;
        }
        // Consume -isysroot and the path that follows it.
        if *token == "-isysroot" {
            skip_next = true;
            continue;
        }
        // Inline -isysroot=<path> form.
        if token.starts_with("-isysroot=") {
            continue;
        }
        // Strip Xcode-version-specific availability warning flags.
        if *token == "-Werror=unguarded-availability-new"
            || *token == "-Wno-unguarded-availability-new"
        {
            continue;
        }
        // Strip bare words that are not compiler flags (no leading '-').
        // These are stray tokens from PG's CFLAGS that confuse the compiler
        // when passed as positional arguments (clang treats them as input files).
        if !token.starts_with('-') {
            eprintln!("build.rs: pgvector: stripping bare CFLAGS token: {token:?}");
            continue;
        }
        out.push(*token);
    }

    out.join(" ")
}

/// Walk the staging directory and return all regular files under lib/ and
/// share/extension/ that belong to pgvector.
fn collect_pgvector_files(staging: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_recursive(staging, &mut files);
    files
}

fn collect_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_recursive(&path, out);
        } else if path.is_file() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name == "vector.control"
                || name.ends_with(".so")
                || name.ends_with(".dylib")
                || (name.starts_with("vector--") && name.ends_with(".sql"))
                || name == "halfvec.control"
                || name == "sparsevec.control"
            {
                out.push(path);
            }
        }
    }
}

/// Produce a tar.gz with paths normalised to the installation root layout:
///   `lib/vector.so` (or `lib/vector.dylib` on macOS)
///   `share/extension/vector.control`
///   `share/extension/vector--*.sql`
fn pack_pgvector_files(files: &[PathBuf], _staging: &Path, dest: &Path, target: &str) {
    let gz = flate2::write::GzEncoder::new(
        std::fs::File::create(dest).expect("create pgvector archive"),
        flate2::Compression::best(),
    );
    let mut builder = tar::Builder::new(gz);

    for file in files {
        let name = file.file_name().unwrap_or_default().to_string_lossy();
        // Determine archive path inside the installation root.
        let archive_path = if name.ends_with(".so") || name.ends_with(".dylib") {
            // Shared lib → lib/
            let _ = target; // suppress unused warning on non-unix paths
            format!("lib/{name}")
        } else {
            // SQL scripts and .control files → share/extension/
            format!("share/extension/{name}")
        };
        let mut f = std::fs::File::open(file).expect("open pgvector file");
        let metadata = f.metadata().expect("metadata");
        let mut header = tar::Header::new_gnu();
        header.set_size(metadata.len());
        header.set_mode(0o755);
        header.set_cksum();
        builder
            .append_data(&mut header, &archive_path, &mut f)
            .expect("append pgvector file to archive");
        eprintln!("  packed: {archive_path}  (from {})", file.display());
    }
    let gz = builder.into_inner().expect("finish tar");
    gz.finish().expect("finish gzip");
}

/// Download `url` to `dest` using `curl`.
fn download_with_curl(url: &str, dest: &Path) {
    let status = Command::new("curl")
        .args([
            "--proto",
            "=https",
            "--tlsv1.2",
            "--retry",
            "5",
            "--retry-connrefused",
            "--location",
            "--silent",
            "--show-error",
            "--fail",
            "--output",
            &dest.display().to_string(),
            url,
        ])
        .status()
        .unwrap_or_else(|e| panic!("failed to run curl: {e}"));
    if !status.success() {
        panic!("curl failed (exit {status}) downloading {url}");
    }
}

/// Run a command, printing its output and returning `false` on failure.
fn run_or_warn(cmd: &mut Command, label: &str) -> bool {
    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("build.rs: {label}: failed to spawn: {e}");
            return false;
        }
    };
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!(
            "build.rs: {label} failed (exit {}):\nstdout: {stdout}\nstderr: {stderr}",
            output.status
        );
        return false;
    }
    true
}

/// Verify SHA-256 of `path`. Returns true if matches expected, false on mismatch/missing.
fn verify_sha256(path: &Path, target: &str, expected_fn: fn(&str) -> Option<&'static str>) -> bool {
    use sha2::Digest;
    use std::io::Read;

    let expected = match expected_fn(target) {
        Some(e) => e,
        None => return true,
    };
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; 65536];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buf[..n]),
            Err(_) => return false,
        }
    }
    hex::encode(hasher.finalize()) == expected
}
