use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use brassclaw_host_api::{
    CapabilityDescriptor, CapabilityId, EffectKind, ExtensionId, PermissionMode, ResourceCeiling,
    ResourceEstimate, ResourceProfile, RuntimeKind, TrustClass,
};
use brassclaw_safety::sensitive_paths::is_sensitive_path;
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::sync::RwLock;

use crate::workspace::paths as ws_paths;

// ============================================================================
// V2 Path Validation and Utilities
// ============================================================================

/// Default directories to exclude from recursive operations.
///
/// These are common directories that should be skipped during file traversal
/// operations like grep or glob to avoid performance issues and irrelevant results.
const DEFAULT_EXCLUDED_DIRS: &[&str] = &[".git", "node_modules", "target"];

/// Normalize a path lexically without filesystem access.
///
/// This function resolves "." and ".." components in a path without touching
/// the filesystem. It's used for security checks to prevent directory traversal
/// attacks.
fn normalize_lexical(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                if !components.is_empty() {
                    components.pop();
                }
            }
            std::path::Component::CurDir => {}
            _ => components.push(component),
        }
    }
    components.iter().collect()
}

/// Validate and resolve a path relative to a base directory.
///
/// This function performs security checks to prevent directory traversal attacks:
/// 1. Rejects empty paths
/// 2. Resolves relative paths against the base directory
/// 3. Normalizes the path lexically
/// 4. Ensures the normalized path doesn't escape the base directory
///
/// # Arguments
/// * `raw` - The raw path string to validate
/// * `base` - Optional base directory to resolve relative paths against
///
/// # Returns
/// * `Ok(PathBuf)` - The validated and normalized path
/// * `Err(String)` - Error message if validation fails
fn validate_path(raw: &str, base: Option<&Path>) -> Result<PathBuf, String> {
    let path = Path::new(raw);

    if raw.is_empty() {
        return Err("empty path".to_string());
    }

    // Resolve relative to base if provided
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(base) = base {
        base.join(path)
    } else {
        path.to_path_buf()
    };

    // Normalize and check for path traversal
    let normalized = normalize_lexical(&resolved);
    if let Some(base) = base {
        let base_normalized = normalize_lexical(base);
        if !normalized.starts_with(&base_normalized) {
            return Err(format!("path escapes base directory: {}", raw));
        }
    }

    Ok(normalized)
}

// ============================================================================
// END V2 Path Validation and Utilities
// ============================================================================

pub const PROVIDER_ID: &str = "builtin";

pub const READ_FILE_CAPABILITY_ID: &str = "builtin.read_file";
pub const WRITE_FILE_CAPABILITY_ID: &str = "builtin.write_file";
pub const LIST_DIR_CAPABILITY_ID: &str = "builtin.list_dir";
pub const APPLY_PATCH_CAPABILITY_ID: &str = "builtin.apply_patch";
pub const GLOB_CAPABILITY_ID: &str = "builtin.glob";
pub const GREP_CAPABILITY_ID: &str = "builtin.grep";
pub const FILE_UNDO_CAPABILITY_ID: &str = "builtin.file_undo";

const DEFAULT_LINE_LIMIT: usize = 2000;
const MAX_READ_SIZE: u64 = 10 * 1024 * 1024;
const MAX_WRITE_SIZE: usize = 5 * 1024 * 1024;
const MAX_DIR_ENTRIES: usize = 500;
const MAX_UNDO_SNAPSHOTS: usize = 20;
const MAX_SNAPSHOT_FILE_SIZE: u64 = 2 * 1024 * 1024;
const DEFAULT_GLOB_MAX_RESULTS: usize = 200;
const DEFAULT_GREP_HEAD_LIMIT: usize = 250;
const MAX_GREP_OUTPUT_SIZE: usize = 64 * 1024;

const DEFAULT_OUTPUT_BYTES: u64 = 16 * 1024;
const MAX_OUTPUT_BYTES: u64 = 1_048_576;
const DEFAULT_WALL_CLOCK_MS: u64 = 100;
const MAX_WALL_CLOCK_MS: u64 = 5_000;

const BLOCKED_DEVICE_PATHS: &[&str] = &[
    "/dev/zero",
    "/dev/urandom",
    "/dev/random",
    "/proc/kcore",
    "/proc/kmem",
    "/dev/null",
    "/dev/stdin",
    "/dev/stdout",
    "/dev/stderr",
];

const WORKSPACE_FILES: &[&str] = &[
    ws_paths::HEARTBEAT,
    ws_paths::MEMORY,
    ws_paths::IDENTITY,
    ws_paths::SOUL,
    ws_paths::AGENTS,
    ws_paths::USER,
    ws_paths::README,
];

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct FilesystemCapabilityError {
    pub message: String,
    pub is_input_error: bool,
}

impl FilesystemCapabilityError {
    fn input(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_input_error: true,
        }
    }

    fn operation(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_input_error: false,
        }
    }
}

#[derive(Debug, Clone)]
struct FileSnapshot {
    path: PathBuf,
    content_before: Option<Vec<u8>>,
    tool_name: String,
}

pub struct FilesystemCapabilityState {
    undo_history: RwLock<VecDeque<FileSnapshot>>,
}

impl Default for FilesystemCapabilityState {
    fn default() -> Self {
        Self {
            undo_history: RwLock::new(VecDeque::new()),
        }
    }
}

impl std::fmt::Debug for FilesystemCapabilityState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilesystemCapabilityState").finish()
    }
}

impl FilesystemCapabilityState {
    pub fn new() -> Self {
        Self::default()
    }

    async fn snapshot(&self, path: &Path, tool_name: &str) {
        let content_before = match tokio::fs::read(path).await {
            Ok(bytes) => {
                if bytes.len() as u64 > MAX_SNAPSHOT_FILE_SIZE {
                    return;
                }
                Some(bytes)
            }
            Err(_) => None,
        };
        let canonical = path.canonicalize().unwrap_or_else(|_| {
            path.parent()
                .and_then(|p| p.canonicalize().ok())
                .map(|cp| cp.join(path.file_name().unwrap_or_default()))
                .unwrap_or_else(|| normalize_lexical(path))
        });
        let mut history = self.undo_history.write().await;
        if history.len() >= MAX_UNDO_SNAPSHOTS {
            history.pop_front();
        }
        history.push_back(FileSnapshot {
            path: canonical,
            content_before,
            tool_name: tool_name.to_string(),
        });
    }
}

pub struct FilesystemContext {
    pub base_dir: PathBuf,
    pub state: Arc<FilesystemCapabilityState>,
}

fn resource_profile() -> Option<ResourceProfile> {
    Some(ResourceProfile {
        default_estimate: ResourceEstimate {
            wall_clock_ms: Some(DEFAULT_WALL_CLOCK_MS),
            output_bytes: Some(DEFAULT_OUTPUT_BYTES),
            ..ResourceEstimate::default()
        },
        hard_ceiling: Some(ResourceCeiling {
            max_usd: None,
            max_input_tokens: None,
            max_output_tokens: None,
            max_wall_clock_ms: Some(MAX_WALL_CLOCK_MS),
            max_output_bytes: Some(MAX_OUTPUT_BYTES),
            sandbox: None,
        }),
    })
}

fn make_descriptor(
    id: &str,
    description: &str,
    effects: Vec<EffectKind>,
    parameters_schema: Value,
    default_permission: PermissionMode,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: CapabilityId::new(id).expect("valid capability id"),
        provider: ExtensionId::new(PROVIDER_ID).expect("valid provider id"),
        runtime: RuntimeKind::FirstParty,
        trust_ceiling: TrustClass::Sandbox,
        description: description.to_string(),
        parameters_schema,
        effects,
        default_permission,
        runtime_credentials: Vec::new(),
        resource_profile: resource_profile(),
    }
}

pub fn read_file_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        READ_FILE_CAPABILITY_ID,
        "Read a file through scoped mounts",
        vec![EffectKind::ReadFilesystem],
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file to read" },
                "offset": { "type": "integer", "minimum": 0, "description": "1-based starting line; 0 starts at the beginning" },
                "limit": { "type": "integer", "minimum": 0, "description": "Maximum lines to return" }
            },
            "required": ["path"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn write_file_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        WRITE_FILE_CAPABILITY_ID,
        "Write content to a file through scoped mounts",
        vec![EffectKind::WriteFilesystem],
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file to write" },
                "content": { "type": "string", "description": "Complete file content" }
            },
            "required": ["path", "content"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn list_dir_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        LIST_DIR_CAPABILITY_ID,
        "List directory contents through scoped mounts",
        vec![EffectKind::ReadFilesystem],
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory path. Defaults to the workspace root." },
                "recursive": { "type": "boolean", "description": "Whether to list recursively" },
                "max_depth": { "type": "integer", "minimum": 0, "description": "Maximum recursive depth" }
            },
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn apply_patch_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        APPLY_PATCH_CAPABILITY_ID,
        "Apply exact/fuzzy search-replace edits through scoped mounts",
        vec![EffectKind::ReadFilesystem, EffectKind::WriteFilesystem],
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path to patch" },
                "old_string": { "type": "string", "description": "Exact text to replace" },
                "new_string": { "type": "string", "description": "Replacement text" },
                "replace_all": { "type": "boolean", "description": "Replace every match instead of exactly one" }
            },
            "required": ["path", "old_string", "new_string"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn glob_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        GLOB_CAPABILITY_ID,
        "Find files under a scoped directory matching a glob pattern",
        vec![EffectKind::ReadFilesystem],
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Glob pattern relative to path" },
                "path": { "type": "string", "description": "Root path. Defaults to the workspace root." },
                "max_results": { "type": "integer", "minimum": 0 }
            },
            "required": ["pattern"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn grep_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        GREP_CAPABILITY_ID,
        "Search scoped file contents with regex",
        vec![EffectKind::ReadFilesystem],
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Regular expression to search for" },
                "path": { "type": "string", "description": "File or directory path. Defaults to the workspace root." },
                "glob": { "type": "string", "description": "Optional glob filter relative to path" },
                "type_filter": { "type": "string", "description": "Optional file type filter" },
                "output_mode": {
                    "type": "string",
                    "enum": ["content", "files_with_matches", "count"],
                    "description": "Output mode. Defaults to files_with_matches."
                },
                "case_insensitive": { "type": "boolean" },
                "multiline": { "type": "boolean" },
                "context": { "type": "integer", "minimum": 0 },
                "before_context": { "type": "integer", "minimum": 0 },
                "after_context": { "type": "integer", "minimum": 0 },
                "head_limit": { "type": "integer", "minimum": 0 },
                "offset": { "type": "integer", "minimum": 0 }
            },
            "required": ["pattern"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn file_undo_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        FILE_UNDO_CAPABILITY_ID,
        "Undo the most recent modification to a file by restoring its previous content",
        vec![EffectKind::WriteFilesystem],
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to restore"
                }
            },
            "required": ["path"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn descriptors() -> Vec<CapabilityDescriptor> {
    vec![
        read_file_descriptor(),
        write_file_descriptor(),
        list_dir_descriptor(),
        apply_patch_descriptor(),
        glob_descriptor(),
        grep_descriptor(),
        file_undo_descriptor(),
    ]
}

fn require_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, FilesystemCapabilityError> {
    params.get(key).and_then(Value::as_str).ok_or_else(|| {
        FilesystemCapabilityError::input(format!("missing required parameter: {key}"))
    })
}

fn resolve_path(base: &Path, raw: &str) -> Result<PathBuf, FilesystemCapabilityError> {
    validate_path(raw, Some(base)).map_err(|e| {
        FilesystemCapabilityError::operation(format!("path validation failed for '{}': {}", raw, e))
    })
}

fn check_sensitive(path: &Path) -> Result<(), FilesystemCapabilityError> {
    if is_sensitive_path(path) {
        return Err(FilesystemCapabilityError::operation(
            "Access denied: this file may contain credentials. \
             Use `secret_list` and `secret_create` to manage credentials securely."
                .to_string(),
        ));
    }
    Ok(())
}

fn check_blocked_device(path: &Path) -> Result<(), FilesystemCapabilityError> {
    let resolved_str = path.to_string_lossy();
    if BLOCKED_DEVICE_PATHS
        .iter()
        .any(|p| resolved_str.starts_with(p))
        || (resolved_str.starts_with("/proc/") && resolved_str.contains("/fd/"))
    {
        return Err(FilesystemCapabilityError::operation(format!(
            "Reading device/proc paths is not allowed: {}",
            path.display()
        )));
    }
    Ok(())
}

fn check_binary(probe: &[u8]) -> Result<(), FilesystemCapabilityError> {
    if probe.is_empty() {
        return Ok(());
    }
    let is_utf16le = probe.len() >= 2 && probe[0] == 0xFF && probe[1] == 0xFE;
    if !is_utf16le && probe.contains(&0) {
        return Err(FilesystemCapabilityError::operation(
            "File appears to be binary (contains null bytes)".to_string(),
        ));
    }
    Ok(())
}

fn is_workspace_path(path: &str) -> bool {
    let filename = Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);
    WORKSPACE_FILES.contains(&filename)
        || path.starts_with("daily/")
        || path.starts_with("context/")
}

fn is_excluded_path(path: &Path) -> bool {
    path.components().any(|c| {
        matches!(c, std::path::Component::Normal(name)
            if name.to_str().is_some_and(|s| DEFAULT_EXCLUDED_DIRS.contains(&s)))
    })
}

pub async fn execute_read_file(
    params: &Value,
    ctx: &FilesystemContext,
) -> Result<Value, FilesystemCapabilityError> {
    let path_str = require_str(params, "path")?;
    let path = resolve_path(&ctx.base_dir, path_str)?;

    check_sensitive(&path)?;
    check_blocked_device(&path)?;

    let metadata = tokio::fs::metadata(&path).await.map_err(|e| {
        FilesystemCapabilityError::operation(format!("cannot read '{}': {}", path_str, e))
    })?;

    if metadata.len() > MAX_READ_SIZE {
        return Err(FilesystemCapabilityError::operation(format!(
            "file too large ({} bytes, max {}). Use offset/limit for partial reads.",
            metadata.len(),
            MAX_READ_SIZE
        )));
    }

    {
        let probe_size = 8192u64.min(metadata.len()) as usize;
        if probe_size > 0 {
            let mut f = tokio::fs::File::open(&path).await.map_err(|e| {
                FilesystemCapabilityError::operation(format!("cannot open '{}': {}", path_str, e))
            })?;
            let mut probe = vec![0u8; probe_size];
            let n = f.read(&mut probe).await.map_err(|e| {
                FilesystemCapabilityError::operation(format!("cannot read '{}': {}", path_str, e))
            })?;
            check_binary(&probe[..n])?;
        }
    }

    let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
        FilesystemCapabilityError::operation(format!("failed to read '{}': {}", path_str, e))
    })?;

    let offset = params.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let limit = params
        .get("limit")
        .and_then(Value::as_u64)
        .map(|v| v as usize);

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();
    let start = if offset > 0 {
        offset.saturating_sub(1)
    } else {
        0
    };
    let end = match limit {
        Some(l) => (start + l).min(total_lines),
        None => (start + DEFAULT_LINE_LIMIT).min(total_lines),
    };
    let selected: Vec<String> = lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{:>6}\u{2192}{}", start + i + 1, line))
        .collect();

    Ok(json!({
        "content": selected.join("\n"),
        "total_lines": total_lines,
        "lines_shown": end - start,
        "start_line": start + 1,
        "path": path.display().to_string()
    }))
}

pub async fn execute_write_file(
    params: &Value,
    ctx: &FilesystemContext,
) -> Result<Value, FilesystemCapabilityError> {
    let path_str = require_str(params, "path")?;

    if is_workspace_path(path_str) {
        return Err(FilesystemCapabilityError::input(format!(
            "'{}' is a workspace memory file. Use the memory_write tool instead of write_file.",
            path_str
        )));
    }

    let content = require_str(params, "content")?;

    if content.len() > MAX_WRITE_SIZE {
        return Err(FilesystemCapabilityError::operation(format!(
            "content too large ({} bytes, max {})",
            content.len(),
            MAX_WRITE_SIZE
        )));
    }

    let path = resolve_path(&ctx.base_dir, path_str).or_else(|_| {
        let p = if Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else {
            ctx.base_dir.join(path_str)
        };
        let normalized = normalize_lexical(&p);
        let base_canonical = ctx
            .base_dir
            .canonicalize()
            .unwrap_or_else(|_| normalize_lexical(&ctx.base_dir));
        if !normalized.starts_with(&base_canonical) {
            return Err(FilesystemCapabilityError::operation(format!(
                "path escapes sandbox: {}",
                path_str
            )));
        }
        Ok(normalized)
    })?;

    check_sensitive(&path)?;

    ctx.state.snapshot(&path, "write_file").await;

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            FilesystemCapabilityError::operation(format!("failed to create directories: {}", e))
        })?;
    }

    tokio::fs::write(&path, content).await.map_err(|e| {
        FilesystemCapabilityError::operation(format!("failed to write '{}': {}", path_str, e))
    })?;

    Ok(json!({
        "status": "ok",
        "path": path.display().to_string(),
        "bytes_written": content.len()
    }))
}

pub async fn execute_list_dir(
    params: &Value,
    ctx: &FilesystemContext,
) -> Result<Value, FilesystemCapabilityError> {
    let path_str = params.get("path").and_then(Value::as_str).unwrap_or(".");
    let recursive = params
        .get("recursive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let max_depth = params
        .get("max_depth")
        .and_then(Value::as_u64)
        .map(|v| v as usize);
    let path = resolve_path(&ctx.base_dir, path_str)?;

    check_sensitive(&path)?;

    let mut entries = Vec::new();
    list_dir_recursive(
        &path,
        &path,
        recursive,
        max_depth.unwrap_or(3),
        0,
        &mut entries,
    )
    .await;

    entries.sort_by(|a, b| {
        let a_name = a.get("name").and_then(Value::as_str).unwrap_or("");
        let b_name = b.get("name").and_then(Value::as_str).unwrap_or("");
        a_name.cmp(b_name)
    });

    Ok(json!({
        "path": path.display().to_string(),
        "entries": entries,
        "count": entries.len()
    }))
}

async fn list_dir_recursive(
    root: &Path,
    dir: &Path,
    recursive: bool,
    max_depth: usize,
    current_depth: usize,
    entries: &mut Vec<Value>,
) {
    let Ok(mut read_dir) = tokio::fs::read_dir(dir).await else {
        return;
    };
    while let Ok(Some(entry)) = read_dir.next_entry().await {
        if entries.len() >= MAX_DIR_ENTRIES {
            break;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry.file_type().await.ok();
        let is_dir = file_type.as_ref().is_some_and(|ft| ft.is_dir());
        let entry_path = entry.path();
        let relative = entry_path
            .strip_prefix(root)
            .unwrap_or(&entry_path)
            .to_string_lossy()
            .to_string();

        if is_dir && DEFAULT_EXCLUDED_DIRS.contains(&name.as_str()) {
            continue;
        }

        entries.push(json!({
            "name": relative,
            "type": if is_dir { "directory" } else { "file" }
        }));

        if recursive && is_dir && current_depth < max_depth {
            Box::pin(list_dir_recursive(
                root,
                &entry_path,
                recursive,
                max_depth,
                current_depth + 1,
                entries,
            ))
            .await;
        }
    }
}

pub async fn execute_apply_patch(
    params: &Value,
    ctx: &FilesystemContext,
) -> Result<Value, FilesystemCapabilityError> {
    let path_str = require_str(params, "path")?;
    let old_string = require_str(params, "old_string")?;
    let new_string = require_str(params, "new_string")?;
    let replace_all = params
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let path = resolve_path(&ctx.base_dir, path_str)?;

    check_sensitive(&path)?;

    let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
        FilesystemCapabilityError::operation(format!("failed to read '{}': {}", path_str, e))
    })?;

    if !content.contains(old_string) {
        return Err(FilesystemCapabilityError::operation(
            "old_string not found in file".to_string(),
        ));
    }

    ctx.state.snapshot(&path, "apply_patch").await;

    let new_content = if replace_all {
        content.replace(old_string, new_string)
    } else {
        content.replacen(old_string, new_string, 1)
    };

    let replacements = if replace_all {
        content.matches(old_string).count()
    } else {
        1
    };

    tokio::fs::write(&path, &new_content).await.map_err(|e| {
        FilesystemCapabilityError::operation(format!("failed to write '{}': {}", path_str, e))
    })?;

    Ok(json!({
        "status": "ok",
        "path": path.display().to_string(),
        "replacements": replacements
    }))
}

pub async fn execute_glob(
    params: &Value,
    ctx: &FilesystemContext,
) -> Result<Value, FilesystemCapabilityError> {
    let pattern = require_str(params, "pattern")?;
    let root = params.get("path").and_then(Value::as_str).unwrap_or(".");
    let max_results = params
        .get("max_results")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_GLOB_MAX_RESULTS as u64) as usize;

    if Path::new(pattern).is_absolute() {
        return Err(FilesystemCapabilityError::input(
            "Absolute glob patterns are not allowed. Use the 'path' parameter to set the search root.".to_string(),
        ));
    }
    if Path::new(pattern)
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(FilesystemCapabilityError::input(
            "Glob patterns containing parent directory traversal ('..') are not allowed."
                .to_string(),
        ));
    }

    let search_root = resolve_path(&ctx.base_dir, root)?;
    check_sensitive(&search_root)?;

    let full_pattern = search_root.join(pattern);
    let full_pattern_str = full_pattern.to_string_lossy().to_string();

    let search_root_clone = search_root.clone();
    let files = tokio::task::spawn_blocking(move || {
        let options = glob::MatchOptions {
            case_sensitive: true,
            require_literal_separator: false,
            require_literal_leading_dot: false,
        };

        let entries = glob::glob_with(&full_pattern_str, options).map_err(|e| {
            FilesystemCapabilityError::input(format!("invalid glob pattern: {}", e))
        })?;

        let mut files: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
        for entry in entries {
            let path = match entry {
                Ok(p) => p,
                Err(_) => continue,
            };

            if path.is_dir() {
                continue;
            }

            let Ok(relative) = path.strip_prefix(&search_root_clone) else {
                continue;
            };
            if is_excluded_path(relative) || is_sensitive_path(&path) {
                continue;
            }

            let mtime = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);

            files.push((path, mtime));
        }

        Ok::<_, FilesystemCapabilityError>(files)
    })
    .await
    .map_err(|e| FilesystemCapabilityError::operation(format!("glob task failed: {}", e)))??;

    let mut files = files;
    files.sort_by_key(|entry| std::cmp::Reverse(entry.1));

    let truncated = files.len() > max_results;
    files.truncate(max_results);

    let file_paths: Vec<String> = files
        .iter()
        .map(|(p, _)| {
            p.strip_prefix(&search_root)
                .unwrap_or(p)
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    Ok(json!({
        "files": file_paths,
        "count": file_paths.len(),
        "truncated": truncated,
        "pattern": pattern,
        "root": search_root.display().to_string()
    }))
}

pub async fn execute_grep(
    params: &Value,
    ctx: &FilesystemContext,
) -> Result<Value, FilesystemCapabilityError> {
    let pattern = require_str(params, "pattern")?;
    let path_str = params.get("path").and_then(Value::as_str).unwrap_or(".");
    let glob_filter = params.get("glob").and_then(Value::as_str);
    let output_mode = params
        .get("output_mode")
        .and_then(Value::as_str)
        .unwrap_or("files_with_matches");
    let context_lines = params.get("context").and_then(Value::as_u64);
    let before_context = params.get("before_context").and_then(Value::as_u64);
    let after_context = params.get("after_context").and_then(Value::as_u64);
    let case_insensitive = params
        .get("case_insensitive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let head_limit = params
        .get("head_limit")
        .and_then(Value::as_u64)
        .map(|v| v as usize);
    let offset = params.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let multiline = params
        .get("multiline")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let type_filter = params.get("type_filter").and_then(Value::as_str);

    let search_path = resolve_path(&ctx.base_dir, path_str)?;
    check_sensitive(&search_path)?;

    let mut cmd = tokio::process::Command::new("rg");
    cmd.env_clear();
    for key in &["PATH", "HOME", "LANG", "TERM"] {
        if let Ok(val) = std::env::var(key) {
            cmd.env(key, val);
        }
    }

    cmd.arg("--color").arg("never");
    cmd.arg("--no-heading");
    cmd.arg("--glob").arg("!.git");
    cmd.arg("--glob").arg("!node_modules");
    cmd.arg("--glob").arg("!target");

    match output_mode {
        "files_with_matches" => {
            cmd.arg("--files-with-matches");
        }
        "count" => {
            cmd.arg("--count");
        }
        "content" => {
            cmd.arg("-n");
        }
        _ => {
            return Err(FilesystemCapabilityError::input(format!(
                "Invalid output_mode '{}'. Must be: content, files_with_matches, or count",
                output_mode
            )));
        }
    }

    if let Some(c) = context_lines {
        cmd.arg("-C").arg(c.to_string());
    } else {
        if let Some(b) = before_context {
            cmd.arg("-B").arg(b.to_string());
        }
        if let Some(a) = after_context {
            cmd.arg("-A").arg(a.to_string());
        }
    }

    if case_insensitive {
        cmd.arg("-i");
    }
    if multiline {
        cmd.arg("-U");
        cmd.arg("--multiline-dotall");
    }

    if let Some(g) = glob_filter {
        cmd.arg("--glob").arg(g);
    }
    if let Some(t) = type_filter {
        cmd.arg("--type").arg(t);
    }

    cmd.arg("-e").arg(pattern);
    cmd.arg(&search_path);

    let output = tokio::time::timeout(Duration::from_secs(30), cmd.output())
        .await
        .map_err(|_| FilesystemCapabilityError::operation("grep timed out after 30s".to_string()))?
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FilesystemCapabilityError::operation("ripgrep (rg) is not installed".to_string())
            } else {
                FilesystemCapabilityError::operation(format!("failed to execute rg: {}", e))
            }
        })?;

    if output.status.code() == Some(2) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(FilesystemCapabilityError::operation(format!(
            "ripgrep error: {}",
            stderr.trim()
        )));
    }

    let raw_output = String::from_utf8_lossy(&output.stdout);
    let truncated_output = if raw_output.len() > MAX_GREP_OUTPUT_SIZE {
        let mut end = MAX_GREP_OUTPUT_SIZE;
        while end > 0 && !raw_output.is_char_boundary(end) {
            end -= 1;
        }
        &raw_output[..end]
    } else {
        &raw_output
    };

    let lines: Vec<&str> = truncated_output.lines().collect();
    let effective_limit = match head_limit {
        Some(0) => lines.len(),
        Some(n) => n,
        None => DEFAULT_GREP_HEAD_LIMIT,
    };

    let paginated: Vec<&str> = lines
        .iter()
        .skip(offset)
        .take(effective_limit)
        .copied()
        .collect();
    let was_truncated =
        raw_output.len() > MAX_GREP_OUTPUT_SIZE || lines.len() > offset + effective_limit;

    let result = match output_mode {
        "files_with_matches" => {
            let file_paths: Vec<String> = paginated
                .iter()
                .filter(|l| !l.is_empty())
                .map(|line| {
                    Path::new(line.trim())
                        .strip_prefix(&search_path)
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| line.trim().to_string())
                })
                .collect();
            json!({
                "files": file_paths,
                "count": file_paths.len(),
                "truncated": was_truncated
            })
        }
        "count" => {
            let mut counts: Vec<Value> = Vec::new();
            let mut total: u64 = 0;
            for line in &paginated {
                if let Some((file, count_str)) = line.rsplit_once(':') {
                    let count = count_str.trim().parse::<u64>().unwrap_or(0);
                    let relative = Path::new(file)
                        .strip_prefix(&search_path)
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| file.to_string());
                    total += count;
                    counts.push(json!({"file": relative, "count": count}));
                }
            }
            json!({
                "counts": counts,
                "total_matches": total,
                "truncated": was_truncated
            })
        }
        _ => {
            let content_str = paginated.join("\n");
            json!({
                "content": content_str,
                "line_count": paginated.len(),
                "truncated": was_truncated
            })
        }
    };

    Ok(result)
}

pub async fn execute_file_undo(
    params: &Value,
    ctx: &FilesystemContext,
) -> Result<Value, FilesystemCapabilityError> {
    let path_str = require_str(params, "path")?;
    let path = resolve_path(&ctx.base_dir, path_str)?;

    let mut history = ctx.state.undo_history.write().await;

    // Canonicalize the lookup path so it matches the canonicalized path stored
    // during snapshot (which uses path.canonicalize()). On macOS, /tmp resolves
    // to /private/tmp, so a lexical match would fail without this step.
    let canonical_lookup = path.canonicalize().unwrap_or(path.clone());

    let snapshot_idx = history
        .iter()
        .rposition(|s| s.path == canonical_lookup)
        .ok_or_else(|| {
            FilesystemCapabilityError::operation(format!(
                "no undo history found for '{}'",
                path_str
            ))
        })?;

    let snapshot = history.remove(snapshot_idx).unwrap();

    match &snapshot.content_before {
        Some(content) => {
            tokio::fs::write(&path, content).await.map_err(|e| {
                FilesystemCapabilityError::operation(format!(
                    "failed to restore '{}': {}",
                    path_str, e
                ))
            })?;
            Ok(json!({
                "status": "ok",
                "action": "restored",
                "path": path.display().to_string(),
                "undone_tool": snapshot.tool_name,
                "bytes_restored": content.len()
            }))
        }
        None => {
            if path.exists() {
                tokio::fs::remove_file(&path).await.map_err(|e| {
                    FilesystemCapabilityError::operation(format!(
                        "failed to remove '{}': {}",
                        path_str, e
                    ))
                })?;
            }
            Ok(json!({
                "status": "ok",
                "action": "removed",
                "path": path.display().to_string(),
                "undone_tool": snapshot.tool_name
            }))
        }
    }
}

pub async fn dispatch(
    capability_id: &str,
    params: &Value,
    ctx: &FilesystemContext,
) -> Result<Value, FilesystemCapabilityError> {
    match capability_id {
        READ_FILE_CAPABILITY_ID => execute_read_file(params, ctx).await,
        WRITE_FILE_CAPABILITY_ID => execute_write_file(params, ctx).await,
        LIST_DIR_CAPABILITY_ID => execute_list_dir(params, ctx).await,
        APPLY_PATCH_CAPABILITY_ID => execute_apply_patch(params, ctx).await,
        GLOB_CAPABILITY_ID => execute_glob(params, ctx).await,
        GREP_CAPABILITY_ID => execute_grep(params, ctx).await,
        FILE_UNDO_CAPABILITY_ID => execute_file_undo(params, ctx).await,
        _ => Err(FilesystemCapabilityError::input(format!(
            "unknown filesystem capability: {}",
            capability_id
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_capability_id(descriptor: &CapabilityDescriptor, expected: &str) {
        assert_eq!(descriptor.id.as_str(), expected);
    }

    fn assert_provider(descriptor: &CapabilityDescriptor) {
        assert_eq!(descriptor.provider.as_str(), PROVIDER_ID);
    }

    fn assert_effects_contain(descriptor: &CapabilityDescriptor, effect: EffectKind) {
        assert!(
            descriptor.effects.contains(&effect),
            "descriptor {} should contain {:?}, has {:?}",
            descriptor.id,
            effect,
            descriptor.effects
        );
    }

    #[test]
    fn read_file_descriptor_correctness() {
        let d = read_file_descriptor();
        assert_capability_id(&d, READ_FILE_CAPABILITY_ID);
        assert_provider(&d);
        assert_eq!(d.runtime, RuntimeKind::FirstParty);
        assert_effects_contain(&d, EffectKind::ReadFilesystem);
        assert_eq!(d.effects.len(), 1);
        assert_eq!(d.default_permission, PermissionMode::Allow);

        let schema = &d.parameters_schema;
        assert!(schema.get("properties").unwrap().get("path").is_some());
        assert!(schema.get("properties").unwrap().get("offset").is_some());
        assert!(schema.get("properties").unwrap().get("limit").is_some());
        let required = schema.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&json!("path")));
    }

    #[test]
    fn write_file_descriptor_correctness() {
        let d = write_file_descriptor();
        assert_capability_id(&d, WRITE_FILE_CAPABILITY_ID);
        assert_provider(&d);
        assert_effects_contain(&d, EffectKind::WriteFilesystem);
        assert_eq!(d.effects.len(), 1);
        assert_eq!(d.default_permission, PermissionMode::Allow);

        let required = d
            .parameters_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("path")));
        assert!(required.contains(&json!("content")));
    }

    #[test]
    fn list_dir_descriptor_correctness() {
        let d = list_dir_descriptor();
        assert_capability_id(&d, LIST_DIR_CAPABILITY_ID);
        assert_provider(&d);
        assert_effects_contain(&d, EffectKind::ReadFilesystem);
        assert_eq!(d.effects.len(), 1);
    }

    #[test]
    fn apply_patch_descriptor_correctness() {
        let d = apply_patch_descriptor();
        assert_capability_id(&d, APPLY_PATCH_CAPABILITY_ID);
        assert_provider(&d);
        assert_effects_contain(&d, EffectKind::ReadFilesystem);
        assert_effects_contain(&d, EffectKind::WriteFilesystem);
        assert_eq!(d.effects.len(), 2);

        let required = d
            .parameters_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("path")));
        assert!(required.contains(&json!("old_string")));
        assert!(required.contains(&json!("new_string")));
    }

    #[test]
    fn glob_descriptor_correctness() {
        let d = glob_descriptor();
        assert_capability_id(&d, GLOB_CAPABILITY_ID);
        assert_provider(&d);
        assert_effects_contain(&d, EffectKind::ReadFilesystem);
        assert_eq!(d.effects.len(), 1);

        let required = d
            .parameters_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("pattern")));
    }

    #[test]
    fn grep_descriptor_correctness() {
        let d = grep_descriptor();
        assert_capability_id(&d, GREP_CAPABILITY_ID);
        assert_provider(&d);
        assert_effects_contain(&d, EffectKind::ReadFilesystem);
        assert_eq!(d.effects.len(), 1);

        let required = d
            .parameters_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("pattern")));
    }

    #[test]
    fn file_undo_descriptor_correctness() {
        let d = file_undo_descriptor();
        assert_capability_id(&d, FILE_UNDO_CAPABILITY_ID);
        assert_provider(&d);
        assert_effects_contain(&d, EffectKind::WriteFilesystem);
        assert_eq!(d.effects.len(), 1);

        let required = d
            .parameters_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("path")));
    }

    #[test]
    fn all_descriptors_count() {
        let all = descriptors();
        assert_eq!(all.len(), 7);
    }

    #[test]
    fn all_descriptors_have_unique_ids() {
        let all = descriptors();
        let ids: std::collections::HashSet<_> =
            all.iter().map(|d| d.id.as_str().to_string()).collect();
        assert_eq!(ids.len(), all.len());
    }

    #[test]
    fn all_descriptors_have_resource_profiles() {
        for d in descriptors() {
            assert!(
                d.resource_profile.is_some(),
                "descriptor {} missing resource profile",
                d.id
            );
        }
    }

    #[tokio::test]
    async fn execute_read_file_happy_path() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, "line1\nline2\nline3\n")
            .await
            .unwrap();

        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        let result = execute_read_file(&json!({"path": file_path.to_str().unwrap()}), &ctx)
            .await
            .unwrap();

        assert_eq!(result.get("total_lines").unwrap().as_u64().unwrap(), 3);
        assert_eq!(result.get("lines_shown").unwrap().as_u64().unwrap(), 3);
    }

    #[tokio::test]
    async fn execute_read_file_with_offset_limit() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let content: String = (1..=100).map(|i| format!("line {i}\n")).collect();
        tokio::fs::write(&file_path, &content).await.unwrap();

        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        let result = execute_read_file(
            &json!({"path": file_path.to_str().unwrap(), "offset": 10, "limit": 5}),
            &ctx,
        )
        .await
        .unwrap();

        assert_eq!(result.get("total_lines").unwrap().as_u64().unwrap(), 100);
        assert_eq!(result.get("lines_shown").unwrap().as_u64().unwrap(), 5);
        assert_eq!(result.get("start_line").unwrap().as_u64().unwrap(), 10);
    }

    #[tokio::test]
    async fn execute_write_file_happy_path() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("out.txt");

        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        let result = execute_write_file(
            &json!({"path": file_path.to_str().unwrap(), "content": "hello world"}),
            &ctx,
        )
        .await
        .unwrap();

        assert_eq!(result.get("status").unwrap().as_str().unwrap(), "ok");
        assert_eq!(result.get("bytes_written").unwrap().as_u64().unwrap(), 11);

        let written = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(written, "hello world");
    }

    #[tokio::test]
    async fn execute_list_dir_happy_path() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(dir.path().join("a.txt"), "a")
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("b.txt"), "b")
            .await
            .unwrap();
        tokio::fs::create_dir(dir.path().join("subdir"))
            .await
            .unwrap();

        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        let result = execute_list_dir(&json!({"path": "."}), &ctx).await.unwrap();
        let entries = result.get("entries").unwrap().as_array().unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[tokio::test]
    async fn execute_apply_patch_happy_path() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("code.rs");
        tokio::fs::write(&file_path, "fn hello() { println!(\"old\"); }")
            .await
            .unwrap();

        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        let result = execute_apply_patch(
            &json!({
                "path": file_path.to_str().unwrap(),
                "old_string": "old",
                "new_string": "new"
            }),
            &ctx,
        )
        .await
        .unwrap();

        assert_eq!(result.get("status").unwrap().as_str().unwrap(), "ok");
        assert_eq!(result.get("replacements").unwrap().as_u64().unwrap(), 1);

        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains("new"));
        assert!(!content.contains("old"));
    }

    #[tokio::test]
    async fn execute_glob_happy_path() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(dir.path().join("a.rs"), "").await.unwrap();
        tokio::fs::write(dir.path().join("b.rs"), "").await.unwrap();
        tokio::fs::write(dir.path().join("c.txt"), "")
            .await
            .unwrap();

        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        let result = execute_glob(&json!({"pattern": "*.rs", "path": "."}), &ctx)
            .await
            .unwrap();

        assert_eq!(result.get("count").unwrap().as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn execute_grep_happy_path() {
        // Skip when ripgrep is not installed (CI environments without rg).
        if std::process::Command::new("rg")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(
            dir.path().join("a.txt"),
            "hello world\nfoo bar\nhello again\n",
        )
        .await
        .unwrap();

        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        let result = execute_grep(
            &json!({"pattern": "hello", "path": "a.txt", "output_mode": "content"}),
            &ctx,
        )
        .await
        .unwrap();

        assert!(result.get("line_count").unwrap().as_u64().unwrap() >= 2);
    }

    #[tokio::test]
    async fn execute_file_undo_happy_path() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("data.txt");
        tokio::fs::write(&file_path, "original content")
            .await
            .unwrap();

        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        execute_write_file(
            &json!({"path": file_path.to_str().unwrap(), "content": "modified content"}),
            &ctx,
        )
        .await
        .unwrap();

        let modified = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(modified, "modified content");

        let result = execute_file_undo(&json!({"path": file_path.to_str().unwrap()}), &ctx)
            .await
            .unwrap();

        assert_eq!(result.get("status").unwrap().as_str().unwrap(), "ok");
        assert_eq!(result.get("action").unwrap().as_str().unwrap(), "restored");

        let restored = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(restored, "original content");
    }

    #[tokio::test]
    async fn execute_file_undo_removes_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("new_file.txt");

        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        execute_write_file(
            &json!({"path": file_path.to_str().unwrap(), "content": "new content"}),
            &ctx,
        )
        .await
        .unwrap();

        assert!(file_path.exists());

        let result = execute_file_undo(&json!({"path": file_path.to_str().unwrap()}), &ctx)
            .await
            .unwrap();

        assert_eq!(result.get("action").unwrap().as_str().unwrap(), "removed");
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn dispatch_routes_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("dispatch_test.txt");
        tokio::fs::write(&file_path, "test content").await.unwrap();

        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        let result = dispatch(
            READ_FILE_CAPABILITY_ID,
            &json!({"path": file_path.to_str().unwrap()}),
            &ctx,
        )
        .await
        .unwrap();

        assert!(result.get("content").is_some());
    }

    #[tokio::test]
    async fn dispatch_unknown_capability_errors() {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        let err = dispatch("builtin.nonexistent", &json!({}), &ctx)
            .await
            .unwrap_err();
        assert!(err.is_input_error);
    }

    #[tokio::test]
    async fn read_file_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        let err = execute_read_file(&json!({"path": "../../etc/passwd"}), &ctx)
            .await
            .unwrap_err();
        assert!(
            err.message.contains("sandbox")
                || err.message.contains("escapes")
                || err.message.contains("validation failed")
        );
    }

    #[tokio::test]
    async fn write_file_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        let err = execute_write_file(
            &json!({"path": "../../tmp/evil.txt", "content": "pwned"}),
            &ctx,
        )
        .await
        .unwrap_err();
        assert!(
            err.message.contains("sandbox")
                || err.message.contains("escapes")
                || err.message.contains("validation failed")
        );
    }

    #[tokio::test]
    async fn write_file_rejects_workspace_paths() {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        let err = execute_write_file(&json!({"path": "MEMORY.md", "content": "overwrite"}), &ctx)
            .await
            .unwrap_err();
        assert!(err.message.contains("workspace") || err.message.contains("memory_write"));
    }

    #[test]
    fn check_binary_rejects_null_bytes() {
        let probe = b"hello\0world";
        let result = check_binary(probe);
        assert!(result.is_err());
    }

    #[test]
    fn check_binary_allows_utf16le() {
        let mut probe = vec![0xFF, 0xFE];
        probe.extend_from_slice(&[0x68, 0x00, 0x65, 0x00]);
        let result = check_binary(&probe);
        assert!(result.is_ok());
    }

    #[test]
    fn check_binary_allows_text() {
        let probe = b"normal text content";
        let result = check_binary(probe);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn list_dir_excludes_default_dirs() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::create_dir(dir.path().join("node_modules"))
            .await
            .unwrap();
        tokio::fs::create_dir(dir.path().join(".git"))
            .await
            .unwrap();
        tokio::fs::create_dir(dir.path().join("src")).await.unwrap();
        tokio::fs::write(dir.path().join("file.txt"), "")
            .await
            .unwrap();

        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        let result = execute_list_dir(&json!({"path": "."}), &ctx).await.unwrap();
        let entries = result.get("entries").unwrap().as_array().unwrap();
        let names: Vec<&str> = entries
            .iter()
            .filter_map(|e| e.get("name").and_then(Value::as_str))
            .collect();
        assert!(!names.contains(&"node_modules"));
        assert!(!names.contains(&".git"));
        assert!(names.contains(&"src"));
        assert!(names.contains(&"file.txt"));
    }

    #[tokio::test]
    async fn glob_excludes_node_modules() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::create_dir_all(dir.path().join("node_modules/pkg"))
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("node_modules/pkg/index.js"), "module")
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("index.js"), "main")
            .await
            .unwrap();

        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        let result = execute_glob(&json!({"pattern": "**/*.js", "path": "."}), &ctx)
            .await
            .unwrap();

        let files = result.get("files").unwrap().as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].as_str().unwrap(), "index.js");
    }

    #[tokio::test]
    async fn glob_rejects_absolute_pattern() {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        let err = execute_glob(&json!({"pattern": "/etc/**/*"}), &ctx)
            .await
            .unwrap_err();
        assert!(err.is_input_error);
    }

    #[tokio::test]
    async fn glob_rejects_traversal_pattern() {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(FilesystemCapabilityState::new());
        let ctx = FilesystemContext {
            base_dir: dir.path().to_path_buf(),
            state,
        };

        let err = execute_glob(&json!({"pattern": "../../**/*"}), &ctx)
            .await
            .unwrap_err();
        assert!(err.is_input_error);
    }
}
