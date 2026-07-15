use std::env;
use std::fs::{self, FileType};
use std::path::{Path, PathBuf};

use brassclaw_skills::{normalize_safe_relative_path, parse_skill_md, validate_skill_name};

type BuildResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Skills carried over from v1 SKILL.md into v2 `MemoryDoc`-backed
/// storage by the Phase-5 migration tool at startup. Each name MUST
/// match a directory under `skills/` at build time — the build script
/// fails loudly if the file is missing, so a renamed or removed v1
/// directory surfaces at compile time instead of silently disappearing
/// at runtime.
const MIGRATED_SKILL_NAMES: &[&str] = &[
    "coding",
    "commit",
    "code-review",
    "github",
    "plan-mode",
    "web-browse",
    "security-review",
    "qa-review",
];

const MIGRATED_SKILLS_CATALOG_PATH: &str = "migrated_skills_catalog.json";

fn main() -> BuildResult<()> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| build_error("brassclaw_reborn_composition lives under crates/"))?;
    embed_reborn_skills(repo_root)?;
    embed_migrated_skills_catalog(repo_root)?;
    Ok(())
}

fn embed_reborn_skills(repo_root: &Path) -> BuildResult<()> {
    let skills_dir = repo_root.join("skills");
    println!("cargo:rerun-if-changed={}", skills_dir.display());

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let summaries_out_path = out_dir.join("embedded_reborn_skill_summaries.json");
    let bundles_out_path = out_dir.join("embedded_reborn_skill_bundles.json");
    if !path_is_real_dir(&skills_dir)? {
        fs::write(summaries_out_path, "[]")?;
        fs::write(bundles_out_path, "[]")?;
        return Ok(());
    }

    let mut skill_summaries = Vec::new();
    let mut skill_bundles = Vec::new();
    let mut entries = fs::read_dir(&skills_dir)?.collect::<Result<Vec<_>, _>>()?;
    entries = entries
        .into_iter()
        .filter_map(|entry| match non_symlink_file_type(&entry) {
            Ok(file_type) if file_type.is_dir() => Some(Ok(entry)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<BuildResult<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let skill_dir = entry.path();
        let skill_md = skill_dir.join("SKILL.md");
        if !path_is_real_file(&skill_md)? {
            continue;
        }

        let dir_name = entry
            .file_name()
            .into_string()
            .map_err(|_| build_error("skill directory name must be UTF-8"))?;
        if !validate_skill_name(&dir_name) {
            return Err(build_error(format!(
                "bundled Reborn skill directory has invalid name `{dir_name}`"
            )));
        }

        let skill_md_content = fs::read_to_string(&skill_md)?;
        let parsed = parse_skill_md(&skill_md_content).map_err(|error| {
            build_error(format!(
                "parse bundled Reborn skill `{dir_name}` at {}: {error}",
                skill_md.display()
            ))
        })?;
        if parsed.manifest.name != dir_name {
            return Err(build_error(format!(
                "bundled Reborn skill `{}` manifest name `{}` must match directory name",
                dir_name, parsed.manifest.name
            )));
        }

        let files = collect_skill_files(&skill_dir)?;
        skill_summaries.push(serde_json::json!({
            "name": parsed.manifest.name,
            "version": parsed.manifest.version,
            "description": parsed.manifest.description,
            "keywords": parsed.manifest.activation.keywords,
            "tags": parsed.manifest.activation.tags,
            "requires_skills": parsed.manifest.requires.skills,
        }));
        skill_bundles.push(serde_json::json!({
            "name": parsed.manifest.name,
            "files": files,
        }));
    }

    fs::write(summaries_out_path, serde_json::to_string(&skill_summaries)?)?;
    fs::write(bundles_out_path, serde_json::to_string(&skill_bundles)?)?;
    Ok(())
}

fn collect_skill_files(skill_dir: &Path) -> BuildResult<Vec<serde_json::Value>> {
    let mut paths = Vec::new();
    collect_files_recursive(skill_dir, &mut paths)?;
    paths.sort();
    paths
        .into_iter()
        .map(|path| skill_file_json(skill_dir, &path))
        .collect()
}

fn collect_files_recursive(dir: &Path, paths: &mut Vec<PathBuf>) -> BuildResult<()> {
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let file_type = non_symlink_file_type(&entry)?;
        if file_type.is_dir() {
            collect_files_recursive(&path, paths)?;
        } else if file_type.is_file() {
            println!("cargo:rerun-if-changed={}", path.display());
            paths.push(path);
        }
    }
    Ok(())
}

fn path_is_real_dir(path: &Path) -> BuildResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(build_error(format!(
            "bundled Reborn skills path must not be a symlink: {}",
            path.display()
        ))),
        Ok(metadata) => Ok(metadata.is_dir()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn path_is_real_file(path: &Path) -> BuildResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(build_error(format!(
            "bundled Reborn skill file must not be a symlink: {}",
            path.display()
        ))),
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn non_symlink_file_type(entry: &fs::DirEntry) -> BuildResult<FileType> {
    let file_type = entry.file_type()?;
    if file_type.is_symlink() {
        return Err(build_error(format!(
            "bundled Reborn skill entry must not be a symlink: {}",
            entry.path().display()
        )));
    }
    Ok(file_type)
}

fn skill_file_json(skill_dir: &Path, source_path: &Path) -> BuildResult<serde_json::Value> {
    let relative_path = source_path.strip_prefix(skill_dir)?;
    let normalized = normalize_safe_relative_path(relative_path)
        .map_err(|error| build_error(format!("skill bundle file path must be safe: {error:?}")))?;
    let path = normalized
        .to_str()
        .ok_or_else(|| build_error("skill bundle file path must be UTF-8"))?
        .replace('\\', "/");
    let bytes = fs::read(source_path)?;
    Ok(serde_json::json!({
        "path": path,
        "bytes": bytes,
    }))
}

fn build_error(reason: impl Into<String>) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        reason.into(),
    ))
}

/// Embed the v1→v2 migration catalog at compile time.
///
/// For each name in `MIGRATED_SKILL_NAMES`, parse the matching
/// `skills/<name>/SKILL.md` and emit a JSON entry containing the
/// parsed `SkillManifest` + body content. The runtime migration tool
/// (`crate::migrated_skills`) reads this catalog and persists any
/// missing entries to the libSQL-backed `MemoryDoc` `Store` at first
/// startup.
///
/// The catalog is embedded as a static JSON blob alongside the
/// existing `/projects/system/skills/<name>/SKILL.md` filesystem
/// materialization (which the loop layer still consults). Phase 6
/// removes the filesystem path entirely; until then both runtime
/// paths are equivalent and the migration catalog is the authoritative
/// v2 source.
fn embed_migrated_skills_catalog(repo_root: &Path) -> BuildResult<()> {
    let skills_dir = repo_root.join("skills");
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let catalog_path = out_dir.join(MIGRATED_SKILLS_CATALOG_PATH);

    let mut entries = Vec::with_capacity(MIGRATED_SKILL_NAMES.len());
    for name in MIGRATED_SKILL_NAMES {
        let skill_md = skills_dir.join(name).join("SKILL.md");
        if !path_is_real_file(&skill_md)? {
            return Err(build_error(format!(
                "migrated skill `{name}` has no SKILL.md at {}",
                skill_md.display()
            )));
        }
        let content = fs::read_to_string(&skill_md)?;
        let parsed = parse_skill_md(&content).map_err(|error| {
            build_error(format!(
                "migrate-skill `{name}` at {}: {error}",
                skill_md.display()
            ))
        })?;
        if parsed.manifest.name != *name {
            return Err(build_error(format!(
                "migrated skill `{name}` manifest.name `{}` does not match directory name",
                parsed.manifest.name
            )));
        }
        entries.push(serde_json::json!({
            "name": name,
            "manifest": parsed.manifest,
            "prompt_content": parsed.prompt_content,
        }));
    }

    println!(
        "cargo:rerun-if-changed={}",
        skills_dir.join("coding/SKILL.md").display()
    );
    fs::write(catalog_path, serde_json::to_string(&entries)?)?;
    Ok(())
}
