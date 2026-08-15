/// Build script for brassclaw_embedded_postgres.
///
/// Downloads the PostgreSQL binary archive for the current compile target at
/// build time and writes it to `$OUT_DIR/postgresql.tar.gz`. The runtime
/// module reads it back via `include_bytes!(env!("EMBEDDED_PG_ARCHIVE"))` and
/// extracts it on first boot — no network access is required at runtime.
///
/// The SHA-256 of the downloaded archive is verified against the compiled-in
/// values so a supply-chain substitution is caught at build time.
use std::path::{Path, PathBuf};

// ── version / URL ─────────────────────────────────────────────────────────────
const PG_VERSION: &str = "16.4.0";
const BASE_URL: &str = "https://github.com/theseus-rs/postgresql-binaries/releases/download";

// ── per-platform checksums (must stay in sync with checksums.rs) ──────────────
fn expected_sha256(target: &str) -> Option<&'static str> {
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

fn main() {
    // Re-run if source files change (archive cached in OUT_DIR is reused across
    // incremental builds once it is present and its checksum is valid).
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=src/checksums.rs");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
    let target = std::env::var("TARGET").expect("TARGET not set");
    let dest = out_dir.join("postgresql.tar.gz");

    // If the target has no known checksum (e.g. Windows CI, unsupported arch)
    // write an empty placeholder so include_bytes! compiles. The runtime will
    // return UnsupportedPlatform before trying to extract the empty bytes.
    if expected_sha256(&target).is_none() {
        if !dest.exists() {
            std::fs::write(&dest, b"").expect("write placeholder archive");
        }
        println!("cargo:rustc-env=EMBEDDED_PG_ARCHIVE={}", dest.display());
        eprintln!(
            "brassclaw build.rs: no pre-built PostgreSQL archive for target {target}; \
             embedded-PG will fall back to runtime download on this platform"
        );
        return;
    }

    // Skip download if the file already exists and its checksum matches.
    if dest.exists() && verify_sha256(&dest, &target) {
        eprintln!(
            "brassclaw build.rs: PostgreSQL archive already cached at {}",
            dest.display()
        );
        println!("cargo:rustc-env=EMBEDDED_PG_ARCHIVE={}", dest.display());
        return;
    }

    let archive_name = format!("postgresql-{PG_VERSION}-{target}.tar.gz");
    let url = format!("{BASE_URL}/{PG_VERSION}/{archive_name}");
    eprintln!("brassclaw build.rs: downloading PostgreSQL {PG_VERSION} for {target}");
    eprintln!("  URL: {url}");

    download_with_curl(&url, &dest);

    if !verify_sha256(&dest, &target) {
        // Remove bad archive so the next build retries.
        let _ = std::fs::remove_file(&dest);
        panic!(
            "SHA-256 mismatch for downloaded PostgreSQL archive for target {target}. \
             The file has been removed; re-run the build to retry the download."
        );
    }

    eprintln!(
        "brassclaw build.rs: PostgreSQL archive verified and cached at {}",
        dest.display()
    );
    println!("cargo:rustc-env=EMBEDDED_PG_ARCHIVE={}", dest.display());
}

/// Download `url` to `dest` using `curl`.
///
/// curl is available on all CI runners (ubuntu-latest, macos-latest) and all
/// mainstream developer workstations.  It handles TLS, redirects, and retries.
fn download_with_curl(url: &str, dest: &Path) {
    let status = std::process::Command::new("curl")
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

/// Verify the SHA-256 of `path` against the expected value for `target`.
/// Returns `true` if the checksum matches, `false` on mismatch or read error.
fn verify_sha256(path: &Path, target: &str) -> bool {
    use sha2::Digest;
    use std::io::Read;

    let expected = match expected_sha256(target) {
        Some(e) => e,
        None => return true, // no expected checksum — treat as valid
    };

    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; 65536];
    loop {
        let n = match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => return false,
        };
        hasher.update(&buf[..n]);
    }
    let digest = hex::encode(hasher.finalize());
    digest == expected
}
