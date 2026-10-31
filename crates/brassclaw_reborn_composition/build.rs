use std::env;
use std::fs;
use std::path::{Path, PathBuf};

type BuildResult<T> = Result<T, Box<dyn std::error::Error>>;

fn write_empty_stub(path: &Path) -> std::io::Result<()> {
    let current = match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    if current.as_deref() != Some(b"[]".as_slice()) {
        fs::write(path, b"[]")?;
    }
    Ok(())
}

fn main() -> BuildResult<()> {
    // No package source/data files feed these fixed stubs. Without this boundary
    // Cargo watches every package file and reruns for unrelated test edits.
    println!("cargo:rerun-if-changed=build.rs");
    // `skills-db` is in the default feature set of `brassclaw_reborn_cli` (added in
    // Phase P.1 Step 1). When active, `bundled_skills.rs` is cfg-gated out; emit
    // empty JSON stubs so the build succeeds without the filesystem walk.
    //
    // The `embed_reborn_skills` + `embed_migrated_skills_catalog` functions that
    // previously did that walk are removed (Phase P.1 Step 3/4 cleanup). The
    // `crate::migrated_skills` consumer module they fed never existed, making
    // `migrated_skills_catalog.json` dead build output. Both functions are gone.
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    write_empty_stub(&out_dir.join("embedded_reborn_skill_summaries.json"))?;
    write_empty_stub(&out_dir.join("embedded_reborn_skill_bundles.json"))?;
    Ok(())
}
