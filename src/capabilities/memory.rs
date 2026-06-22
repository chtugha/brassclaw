use std::path::Path;
use std::sync::Arc;

use brassclaw_host_api::{
    CapabilityDescriptor, CapabilityId, EffectKind, ExtensionId, PermissionMode, ResourceCeiling,
    ResourceEstimate, ResourceProfile, RuntimeKind, TrustClass,
};
use serde_json::{Value, json};
use crate::workspace::{Workspace, paths};

// ============================================================================
// V2 Path Validation
// ============================================================================

/// Normalize a workspace path and check for parent directory traversal.
///
/// Returns `Some(normalized_path)` if the path is valid (no ".." segments),
/// or `None` if the path contains parent directory traversal.
///
/// This is a simple string-based check that doesn't access the filesystem.
pub(crate) fn normalize_workspace_path(path: &str) -> Option<String> {
    // Check for ".." segments which indicate parent directory traversal
    if path.contains("..") {
        return None;
    }
    
    // Normalize by removing redundant slashes and "." segments
    let parts: Vec<&str> = path
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    
    Some(parts.join("/"))
}

/// Check if a path is in a protected orchestrator directory.
///
/// Protected paths are those under `.system/engine/orchestrator/` or the
/// legacy `engine/orchestrator/` directory. These paths should only be
/// modified when orchestrator self-modification is explicitly enabled.
fn is_protected_orchestrator_path(path: &str) -> bool {
    let normalized = normalize_workspace_path(path).unwrap_or_default();
    normalized.starts_with(".system/engine/orchestrator/")
        || normalized.starts_with("engine/orchestrator/")
}

// ============================================================================
// END V2 Path Validation
// ============================================================================

use async_trait::async_trait;

/// Trait for resolving workspace based on user ID (multi-tenant support)
#[async_trait]
pub trait WorkspaceResolver: Send + Sync {
    async fn resolve(&self, user_id: &str) -> Arc<Workspace>;
}

/// Returns a fixed workspace regardless of user ID (single-user mode).
pub struct FixedWorkspaceResolver {
    workspace: Arc<Workspace>,
}

impl FixedWorkspaceResolver {
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self { workspace }
    }
}

#[async_trait]
impl WorkspaceResolver for FixedWorkspaceResolver {
    async fn resolve(&self, _user_id: &str) -> Arc<Workspace> {
        Arc::clone(&self.workspace)
    }
}



pub const PROVIDER_ID: &str = "builtin";
pub const MEMORY_READ_CAPABILITY_ID: &str = "builtin.memory_read";
pub const MEMORY_WRITE_CAPABILITY_ID: &str = "builtin.memory_write";
pub const MEMORY_SEARCH_CAPABILITY_ID: &str = "builtin.memory_search";
pub const MEMORY_TREE_CAPABILITY_ID: &str = "builtin.memory_tree";

const DEFAULT_OUTPUT_BYTES: u64 = 16 * 1024;
const MAX_OUTPUT_BYTES: u64 = 1_048_576;
const DEFAULT_WALL_CLOCK_MS: u64 = 200;
const MAX_WALL_CLOCK_MS: u64 = 10_000;

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct MemoryCapabilityError {
    pub message: String,
    pub is_input_error: bool,
}

impl MemoryCapabilityError {
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

    fn not_authorized(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_input_error: false,
        }
    }
}

pub struct MemoryContext {
    pub resolver: Arc<dyn WorkspaceResolver>,
    pub user_id: String,
    pub user_timezone: String,
    pub llm: Option<Arc<dyn brassclaw_llm::LlmProvider>>,
    pub reasoning_enabled: bool,
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

pub fn memory_read_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        MEMORY_READ_CAPABILITY_ID,
        "Read a file from the workspace memory (database-backed storage). \
         Use this to read files shown by memory_tree or to inspect a document \
         before patching it with memory_write. NOT for local filesystem files \
         (use read_file for those).",
        vec![EffectKind::ReadFilesystem],
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace path to the file (e.g., 'MEMORY.md', 'daily/2024-01-15.md'). Not a local filesystem path."
                },
                "version": {
                    "type": "integer",
                    "description": "Read a specific historical version of the document (omit for current content)"
                },
                "list_versions": {
                    "type": "boolean",
                    "description": "If true, return version history instead of file content",
                    "default": false
                }
            },
            "required": ["path"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn memory_write_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        MEMORY_WRITE_CAPABILITY_ID,
        "Write to persistent memory (database-backed, NOT the local filesystem). \
         Use for important facts, decisions, preferences, workflow docs, or other \
         workspace files that should live in memory rather than on disk.",
        vec![EffectKind::WriteFilesystem],
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "Full content to write. Prefer this for new files or full rewrites."
                },
                "target": {
                    "type": "string",
                    "description": "Where to write: 'memory' for MEMORY.md, 'daily_log' for today's log, 'heartbeat' for HEARTBEAT.md, 'bootstrap' to clear BOOTSTRAP.md, or an exact workspace path.",
                    "default": "daily_log"
                },
                "append": {
                    "type": "boolean",
                    "description": "If true, append to existing content. If false, replace entirely.",
                    "default": true
                },
                "layer": {
                    "type": "string",
                    "description": "Memory layer to write to (e.g. 'private', 'household', 'finance')."
                },
                "force": {
                    "type": "boolean",
                    "description": "Skip privacy classification and write directly to the specified layer.",
                    "default": false
                },
                "metadata": {
                    "type": "object",
                    "description": "Optional metadata to set on the document"
                },
                "old_string": {
                    "type": "string",
                    "description": "When present, switches to patch mode: finds and replaces this exact string."
                },
                "new_string": {
                    "type": "string",
                    "description": "Replacement string for patch mode. Required when old_string is present."
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "If true, replace all occurrences of old_string. Default: false.",
                    "default": false
                }
            },
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn memory_search_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        MEMORY_SEARCH_CAPABILITY_ID,
        "Search past memories, decisions, and context. MUST be called before answering \
         questions about prior work, decisions, dates, people, preferences, or todos. \
         Returns relevant snippets with relevance scores.",
        vec![EffectKind::ReadFilesystem],
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query. Use natural language to describe what you're looking for."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results to return (default: 5, max: 20)",
                    "default": 5,
                    "minimum": 1,
                    "maximum": 20
                },
                "reasoning": {
                    "type": "boolean",
                    "description": "When true, synthesize search results into a coherent summary using LLM reasoning.",
                    "default": false
                }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn memory_tree_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        MEMORY_TREE_CAPABILITY_ID,
        "View the workspace memory structure as a tree (database-backed storage). \
         Use this to discover valid workspace paths before calling memory_read or \
         memory_write.",
        vec![EffectKind::ReadFilesystem],
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Root path to start from (empty string for workspace root)",
                    "default": ""
                },
                "depth": {
                    "type": "integer",
                    "description": "Maximum depth to traverse (1 = immediate children only)",
                    "default": 1,
                    "minimum": 1,
                    "maximum": 10
                }
            },
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn descriptors() -> Vec<CapabilityDescriptor> {
    vec![
        memory_read_descriptor(),
        memory_write_descriptor(),
        memory_search_descriptor(),
        memory_tree_descriptor(),
    ]
}

fn require_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, MemoryCapabilityError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| MemoryCapabilityError::input(format!("missing required parameter: {key}")))
}

fn looks_like_filesystem_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    if Path::new(path).is_absolute() || path.starts_with("~/") {
        return true;
    }
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

fn map_write_err(e: crate::error::WorkspaceError) -> MemoryCapabilityError {
    match e {
        crate::error::WorkspaceError::InjectionRejected { path, reason } => {
            MemoryCapabilityError::not_authorized(format!(
                "content rejected for '{path}': prompt injection detected ({reason})"
            ))
        }
        other => MemoryCapabilityError::operation(format!("Write failed: {other}")),
    }
}

fn self_modify_enabled() -> bool {
    brassclaw_engine::runtime::self_modify_enabled()
}

fn is_protected_py_path(path: &str) -> bool {
    let Some(canonical) = normalize_workspace_path(path) else {
        return false;
    };
    if !canonical.ends_with(".py") {
        return false;
    }
    canonical.starts_with(".system/engine/orchestrator/")
        || canonical.starts_with("engine/orchestrator/")
}

pub async fn execute_memory_read(
    params: &Value,
    ctx: &MemoryContext,
) -> Result<Value, MemoryCapabilityError> {
    let path = require_str(params, "path")?;

    if looks_like_filesystem_path(path) {
        return Err(MemoryCapabilityError::input(format!(
            "'{}' looks like a local filesystem path. memory_read only works with workspace-memory paths. \
             Use read_file for filesystem reads.",
            path
        )));
    }

    let workspace = ctx.resolver.resolve(&ctx.user_id).await;

    let list_versions = params
        .get("list_versions")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let version = match params.get("version").and_then(|v| v.as_i64()) {
        _ if list_versions && params.get("version").is_some() => {
            return Err(MemoryCapabilityError::input(
                "list_versions and version are mutually exclusive".to_string(),
            ));
        }
        Some(v) if v < 1 || v > i64::from(i32::MAX) => {
            return Err(MemoryCapabilityError::input(format!(
                "version must be between 1 and {}, got {v}",
                i32::MAX
            )));
        }
        Some(v) => Some(v as i32),
        None => None,
    };

    let doc = workspace
        .read(path)
        .await
        .map_err(|e| MemoryCapabilityError::operation(format!("Read failed: {}", e)))?;

    if list_versions {
        let versions = workspace
            .list_versions(doc.id, 50)
            .await
            .map_err(|e| MemoryCapabilityError::operation(format!("List versions failed: {}", e)))?;

        return Ok(json!({
            "path": doc.path,
            "versions": versions.iter().map(|v| json!({
                "version": v.version,
                "content_hash": v.content_hash,
                "created_at": v.created_at.to_rfc3339(),
                "changed_by": v.changed_by,
            })).collect::<Vec<_>>(),
            "version_count": versions.len(),
        }));
    }

    if let Some(ver) = version {
        let version_doc = workspace
            .get_version(doc.id, ver)
            .await
            .map_err(|e| MemoryCapabilityError::operation(format!("Get version failed: {}", e)))?;

        return Ok(json!({
            "path": doc.path,
            "version": version_doc.version,
            "content": version_doc.content,
            "content_hash": version_doc.content_hash,
            "created_at": version_doc.created_at.to_rfc3339(),
            "changed_by": version_doc.changed_by,
        }));
    }

    Ok(json!({
        "path": doc.path,
        "content": doc.content,
        "word_count": doc.word_count(),
        "updated_at": doc.updated_at.to_rfc3339(),
    }))
}

pub async fn execute_memory_write(
    params: &Value,
    ctx: &MemoryContext,
) -> Result<Value, MemoryCapabilityError> {
    let target = params
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("daily_log");

    let content = params.get("content").and_then(|v| v.as_str()).unwrap_or("");

    let allows_empty_content = target == "bootstrap";
    let is_patch_mode = params.get("old_string").and_then(|v| v.as_str()).is_some();
    let has_content = !content.trim().is_empty();
    if !is_patch_mode && !has_content && !allows_empty_content {
        return Err(MemoryCapabilityError::input(
            "Either 'content' (for write/append) or 'old_string'+'new_string' (for patch) is required".to_string(),
        ));
    }

    if looks_like_filesystem_path(target) {
        return Err(MemoryCapabilityError::input(format!(
            "'{}' looks like a local filesystem path. memory_write only works with workspace-memory paths. \
             Use write_file for filesystem writes.",
            target
        )));
    }

    if !target.starts_with("orchestrator:")
        && !target.starts_with("prompt:")
        && normalize_workspace_path(target).is_none()
    {
        return Err(MemoryCapabilityError::input(format!(
            "'{}' contains a parent-directory ('..') segment, which is not allowed in workspace paths.",
            target
        )));
    }

    if is_protected_orchestrator_path(target) && !self_modify_enabled() {
        return Err(MemoryCapabilityError::not_authorized(format!(
            "Writing to '{}' is blocked — orchestrator self-modification is disabled. \
             Set ORCHESTRATOR_SELF_MODIFY=true to enable runtime patching.",
            target
        )));
    }

    let workspace = ctx.resolver.resolve(&ctx.user_id).await;

    if target == "bootstrap" {
        workspace
            .write(paths::BOOTSTRAP, "")
            .await
            .map_err(map_write_err)?;
        workspace.mark_bootstrap_completed();
        return Ok(json!({
            "status": "cleared",
            "path": paths::BOOTSTRAP,
            "message": "BOOTSTRAP.md cleared. First-run ritual will not repeat.",
        }));
    }

    let append = params
        .get("append")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let layer = params.get("layer").and_then(|v| v.as_str());
    let force = params
        .get("force")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let tz = crate::timezone::parse_timezone(&ctx.user_timezone).unwrap_or(chrono_tz::Tz::UTC);

    let resolved_path = match target {
        "memory" => paths::MEMORY.to_string(),
        "daily_log" => {
            let now = chrono::Utc::now().with_timezone(&tz);
            format!("daily/{}.md", now.format("%Y-%m-%d"))
        }
        "heartbeat" => paths::HEARTBEAT.to_string(),
        path => path.to_string(),
    };

    let needs_py_validation = is_protected_py_path(&resolved_path);

    let metadata_param = params.get("metadata").filter(|m| m.is_object());
    if let Some(meta) = metadata_param
        && layer.is_none()
    {
        let doc = if is_patch_mode {
            workspace.read_primary(&resolved_path).await.ok()
        } else {
            Some(
                workspace
                    .get_or_create(&resolved_path)
                    .await
                    .map_err(map_write_err)?,
            )
        };
        if let Some(doc) = doc {
            let merged = crate::workspace::DocumentMetadata::merge(&doc.metadata, meta);
            workspace
                .update_metadata(doc.id, &merged)
                .await
                .map_err(map_write_err)?;
        }
    }

    let old_string = params.get("old_string").and_then(|v| v.as_str());
    if let Some(old_str) = old_string {
        if old_str.is_empty() {
            return Err(MemoryCapabilityError::input(
                "old_string cannot be empty".to_string(),
            ));
        }
        if layer.is_some() {
            return Err(MemoryCapabilityError::input(
                "patch mode (old_string/new_string) cannot be combined with layer".to_string(),
            ));
        }
        let new_str = params
            .get("new_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                MemoryCapabilityError::input(
                    "new_string is required when old_string is provided".to_string(),
                )
            })?;
        let replace_all = params
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if needs_py_validation {
            let existing = workspace
                .read(&resolved_path)
                .await
                .map(|d| d.content)
                .unwrap_or_default();
            let preview = if replace_all {
                existing.replace(old_str, new_str)
            } else {
                existing.replacen(old_str, new_str, 1)
            };
            if let Err(reason) = brassclaw_engine::executor::validate_python_syntax(&preview) {
                return Err(MemoryCapabilityError::input(format!(
                    "orchestrator patch has invalid Python syntax: {reason}"
                )));
            }
        }

        let result = workspace
            .patch(&resolved_path, old_str, new_str, replace_all)
            .await
            .map_err(map_write_err)?;

        return Ok(json!({
            "status": "patched",
            "path": resolved_path,
            "replacements": result.replacements,
            "content_length": result.document.content.len(),
        }));
    }

    if needs_py_validation {
        let final_content = if append {
            let existing = workspace
                .read(&resolved_path)
                .await
                .map(|d| d.content)
                .unwrap_or_default();
            format!("{}{}", existing, content)
        } else {
            content.to_string()
        };
        if let Err(reason) = brassclaw_engine::executor::validate_python_syntax(&final_content) {
            return Err(MemoryCapabilityError::input(format!(
                "orchestrator write has invalid Python syntax: {reason}"
            )));
        }
    }

    let _layer_result: Option<(String, bool)> = if let Some(layer_name) = layer {
        let _doc = workspace
            .get_or_create(&resolved_path)
            .await
            .map_err(map_write_err)?;
        if append {
            workspace
                .append_to_layer(layer_name, &resolved_path, content, force)
                .await
                .map_err(map_write_err)?;
        } else {
            workspace
                .write_to_layer(layer_name, &resolved_path, content, force)
                .await
                .map_err(map_write_err)?;
        }
        Some((layer_name.to_string(), false))
    } else {
        match target {
            "memory" => {
                if append {
                    workspace
                        .append(paths::MEMORY, content)
                        .await
                        .map_err(map_write_err)?;
                } else {
                    workspace
                        .write(paths::MEMORY, content)
                        .await
                        .map_err(map_write_err)?;
                }
            }
            "daily_log" => {
                workspace
                    .append_daily_log_tz(content, tz)
                    .await
                    .map_err(map_write_err)?;
            }
            _ => {
                if append {
                    workspace
                        .append(&resolved_path, content)
                        .await
                        .map_err(map_write_err)?;
                } else {
                    workspace
                        .write(&resolved_path, content)
                        .await
                        .map_err(map_write_err)?;
                }
            }
        }
        None
    };

    let mut output = json!({
        "status": "written",
        "path": resolved_path,
        "append": append,
        "content_length": content.len(),
    });
    if let Some((actual_layer, redirected)) = _layer_result {
        output["layer"] = Value::String(actual_layer);
        output["redirected"] = Value::Bool(redirected);
    }

    Ok(output)
}

pub async fn execute_memory_search(
    params: &Value,
    ctx: &MemoryContext,
) -> Result<Value, MemoryCapabilityError> {
    let query = require_str(params, "query")?;

    let limit = params
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(5)
        .min(20) as usize;

    let workspace = ctx.resolver.resolve(&ctx.user_id).await;
    let results = workspace
        .search(query, limit)
        .await
        .map_err(|e| MemoryCapabilityError::operation(format!("Search failed: {}", e)))?;

    let result_count = results.len();

    let use_reasoning = params
        .get("reasoning")
        .and_then(|v| v.as_bool())
        .unwrap_or(ctx.reasoning_enabled);

    if use_reasoning
        && let Some(ref llm) = ctx.llm
        && !results.is_empty()
    {
        let fragments: String = results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                format!(
                    "[{}] (path: {}, score: {:.2})\n{}",
                    i + 1,
                    r.document_path,
                    r.score,
                    r.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let llm_messages = vec![
            brassclaw_llm::ChatMessage::system(include_str!(
                "../../crates/brassclaw_engine/prompts/memory_reasoning_synthesis.md"
            )),
            brassclaw_llm::ChatMessage::user(format!(
                "Query: {query}\n\nMemory fragments:\n{fragments}"
            )),
        ];

        let request = brassclaw_llm::CompletionRequest::new(llm_messages).with_max_tokens(500);

        let reasoning_timeout = std::time::Duration::from_secs(15);
        match tokio::time::timeout(reasoning_timeout, llm.complete(request)).await {
            Ok(Ok(response)) => {
                let sanitizer = brassclaw_safety::Sanitizer::new();
                let sanitized = sanitizer.sanitize(response.content.trim());
                let synthesis = sanitized.content;
                return Ok(json!({
                    "query": query,
                    "synthesis": synthesis,
                    "results": results.iter().map(|r| json!({
                        "content": r.content,
                        "score": r.score,
                        "path": r.document_path,
                        "document_id": r.document_id.to_string(),
                        "is_hybrid_match": r.is_hybrid(),
                    })).collect::<Vec<_>>(),
                    "result_count": result_count,
                    "reasoning_used": true,
                }));
            }
            Ok(Err(_)) | Err(_) => {
                // Fall through to raw results
            }
        }
    }

    Ok(json!({
        "query": query,
        "results": results.iter().map(|r| json!({
            "content": r.content,
            "score": r.score,
            "path": r.document_path,
            "document_id": r.document_id.to_string(),
            "is_hybrid_match": r.is_hybrid(),
        })).collect::<Vec<_>>(),
        "result_count": result_count,
    }))
}

pub async fn execute_memory_tree(
    params: &Value,
    ctx: &MemoryContext,
) -> Result<Value, MemoryCapabilityError> {
    let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");

    let depth = params
        .get("depth")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .clamp(1, 10) as usize;

    let workspace = ctx.resolver.resolve(&ctx.user_id).await;
    let tree = build_tree(&workspace, path, 1, depth).await?;

    Ok(Value::Array(tree))
}

async fn build_tree(
    workspace: &Arc<Workspace>,
    path: &str,
    current_depth: usize,
    max_depth: usize,
) -> Result<Vec<Value>, MemoryCapabilityError> {
    if current_depth > max_depth {
        return Ok(Vec::new());
    }

    let entries = workspace
        .list(path)
        .await
        .map_err(|e| MemoryCapabilityError::operation(format!("Tree failed: {}", e)))?;

    let mut result = Vec::new();
    for entry in entries {
        let display_path = if entry.is_directory {
            format!("{}/", entry.name())
        } else {
            entry.name().to_string()
        };

        if entry.is_directory && current_depth < max_depth {
            let children =
                Box::pin(build_tree(workspace, &entry.path, current_depth + 1, max_depth)).await?;
            if children.is_empty() {
                result.push(Value::String(display_path));
            } else {
                result.push(json!({ display_path: children }));
            }
        } else {
            result.push(Value::String(display_path));
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_read_descriptor_is_valid() {
        let desc = memory_read_descriptor();
        assert_eq!(desc.id.as_str(), MEMORY_READ_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ReadFilesystem));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn memory_write_descriptor_is_valid() {
        let desc = memory_write_descriptor();
        assert_eq!(desc.id.as_str(), MEMORY_WRITE_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::WriteFilesystem));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn memory_search_descriptor_is_valid() {
        let desc = memory_search_descriptor();
        assert_eq!(desc.id.as_str(), MEMORY_SEARCH_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ReadFilesystem));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn memory_tree_descriptor_is_valid() {
        let desc = memory_tree_descriptor();
        assert_eq!(desc.id.as_str(), MEMORY_TREE_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ReadFilesystem));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn descriptors_returns_all_memory() {
        let descs = descriptors();
        assert_eq!(descs.len(), 4);
        let ids: Vec<&str> = descs.iter().map(|d| d.id.as_str()).collect();
        assert!(ids.contains(&MEMORY_READ_CAPABILITY_ID));
        assert!(ids.contains(&MEMORY_WRITE_CAPABILITY_ID));
        assert!(ids.contains(&MEMORY_SEARCH_CAPABILITY_ID));
        assert!(ids.contains(&MEMORY_TREE_CAPABILITY_ID));
    }

    #[test]
    fn detects_filesystem_paths() {
        assert!(looks_like_filesystem_path("/Users/nige/file.md"));
        assert!(looks_like_filesystem_path("C:\\Users\\nige\\file.md"));
        assert!(looks_like_filesystem_path("D:/work/file.md"));
        assert!(looks_like_filesystem_path("~/notes.md"));
    }

    #[test]
    fn allows_workspace_memory_paths() {
        assert!(!looks_like_filesystem_path("MEMORY.md"));
        assert!(!looks_like_filesystem_path("daily/2026-03-11.md"));
        assert!(!looks_like_filesystem_path("projects/alpha/notes.md"));
    }

    #[test]
    fn memory_read_schema_has_required_path() {
        let desc = memory_read_descriptor();
        let required = desc.parameters_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "path"));
    }

    #[test]
    fn memory_write_schema_has_optional_content() {
        let desc = memory_write_descriptor();
        let schema = &desc.parameters_schema;
        assert!(schema["properties"]["content"].is_object());
        assert!(schema["properties"]["target"].is_object());
        assert!(schema["properties"]["old_string"].is_object());
        assert!(schema["properties"]["new_string"].is_object());
    }

    #[test]
    fn memory_search_schema_has_required_query() {
        let desc = memory_search_descriptor();
        let required = desc.parameters_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "query"));
    }

    #[test]
    fn memory_tree_schema_has_depth() {
        let desc = memory_tree_descriptor();
        assert!(desc.parameters_schema["properties"]["depth"].is_object());
        assert_eq!(desc.parameters_schema["properties"]["depth"]["default"], 1);
    }
}
