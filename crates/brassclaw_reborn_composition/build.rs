use std::env;
use std::fs;
use std::path::PathBuf;

type BuildResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> BuildResult<()> {
    // `skills-db` is in the default feature set of `brassclaw_reborn_cli` (added in
    // Phase P.1 Step 1). When active, `bundled_skills.rs` is cfg-gated out; emit
    // empty JSON stubs so the build succeeds without the filesystem walk.
    //
    // The `embed_reborn_skills` + `embed_migrated_skills_catalog` functions that
    // previously did that walk are removed (Phase P.1 Step 3/4 cleanup). The
    // `crate::migrated_skills` consumer module they fed never existed, making
    // `migrated_skills_catalog.json` dead build output. Both functions are gone.
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    fs::write(out_dir.join("embedded_reborn_skill_summaries.json"), "[]")?;
    fs::write(out_dir.join("embedded_reborn_skill_bundles.json"), "[]")?;
    Ok(())
}
