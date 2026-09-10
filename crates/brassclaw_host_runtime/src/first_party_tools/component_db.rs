//! `builtin.component_db` — generic DB read/write/hash/section-extract tool.
//!
//! The one kernel-boundary DB capability for the doc-sync mechanism (Phase P
//! §0.22 / `DOC_CONVERSION_MECHANISM_DESIGN.md §4.0.1`). Provides a small,
//! uniform surface so every future "sync" recipe can recycle it with a
//! different `op`:
//!
//! | op | action |
//! |----|--------|
//! | `compute_hash` | SHA-256 of input text (returned as lowercase hex) |
//! | `read_hash` | read stored `content_hash` for a `reborn_docus` row |
//! | `read_row` | read key fields of a `reborn_docus` row |
//! | `upsert` | insert-or-update a `reborn_docus` row (`validation_status='pending'` always) |
//! | `mark_stale` | mark the base-prompt prefix stale |
//! | `extract_section` | extract a named `## N. title` section from markdown |
//!
//! The concrete Postgres backend lives in `brassclaw_reborn_composition` (to
//! avoid a circular crate dependency). The tool carries an
//! `Option<Arc<dyn ComponentDbBackend>>` injected at wiring time via
//! [`ComponentDbState::with_backend`]. Without injection every op returns
//! [`ComponentDbError::NotWired`].

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_extensions::{CapabilityManifest, ExtensionError};
use brassclaw_host_api::{EffectKind, PermissionMode};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::FirstPartyCapabilityError;

use super::{first_party_capability_manifest, resource_profile};

// ---------------------------------------------------------------------------
// Capability ID
// ---------------------------------------------------------------------------

pub const COMPONENT_DB_CAPABILITY_ID: &str = "builtin.component_db";

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

pub(super) fn manifest() -> Result<CapabilityManifest, ExtensionError> {
    first_party_capability_manifest(
        COMPONENT_DB_CAPABILITY_ID,
        "Generic component-DB tool: compute_hash | read_hash | read_row | upsert | \
         mark_stale | extract_section. The single kernel-boundary DB capability for \
         doc-sync and future sync recipes.",
        // ExternalWrite covers the upsert/mark_stale ops; DispatchCapability covers
        // the pure read ops (read_hash, read_row, compute_hash, extract_section).
        vec![EffectKind::ExternalWrite, EffectKind::DispatchCapability],
        PermissionMode::Allow,
        resource_profile(),
    )
}

// ---------------------------------------------------------------------------
// Backend trait
// ---------------------------------------------------------------------------

/// Scope tuple for a single DB operation (user_id + project_id; tenant_id
/// and agent_id are fixed to the system seed values at this layer).
#[derive(Debug, Clone)]
pub struct ComponentDbScope {
    pub user_id: String,
    pub project_id: String,
}

/// A single `reborn_docus` row (read side).
#[derive(Debug, Clone)]
pub struct ComponentDbRow {
    pub id: String,
    pub name: String,
    pub content: String,
    pub content_hash: String,
    pub validation_status: String,
}

/// Payload for an `upsert` op.
#[derive(Debug, Clone)]
pub struct ComponentDbUpsert {
    pub name: String,
    pub description: String,
    pub content: String,
    pub content_hash: String,
    pub source: String,
    /// Optional: links converted row back to source row.
    pub similarity_parent_id: Option<String>,
    /// Optional: replaces a previous version of the same row.
    pub replaces_id: Option<String>,
    pub consumer_tags: Vec<String>,
}

/// Result of an `upsert` op.
#[derive(Debug, Clone)]
pub struct ComponentDbUpsertResult {
    pub id: String,
    pub content_hash: String,
    pub is_new: bool,
}

/// Errors returned by backend operations.
#[derive(Debug, thiserror::Error)]
pub enum ComponentDbError {
    #[error("component_db backend not wired — inject via BuiltinFirstPartyTools::with_component_db")]
    NotWired,
    #[error("database error: {0}")]
    Db(String),
}

/// The backend trait — implemented by `PgComponentDbBackend` in
/// `brassclaw_reborn_composition`. Uses `#[async_trait]` for dyn-safe async methods.
#[async_trait]
pub trait ComponentDbBackend: Send + Sync {
    /// Compute SHA-256 of `text` and return lowercase hex. Pure operation —
    /// no DB access. Default implementation provided so the Postgres backend
    /// can delegate to the shared Rust impl without re-implementing it.
    fn compute_hash(&self, text: &str) -> String {
        sha256_hex(text)
    }

    /// Extract a named section from markdown (e.g. `## 7. LLM-summary`).
    /// Returns the section body (without the heading line), or `None` if not
    /// found. Default implementation provided — pure string scan.
    fn extract_section(&self, markdown: &str, title: &str) -> Option<String> {
        extract_md_section(markdown, title)
    }

    /// Read the stored `content_hash` for a `reborn_docus` row identified by
    /// `(scope, name)`. Returns `None` when no row exists yet.
    async fn read_hash(
        &self,
        scope: &ComponentDbScope,
        name: &str,
    ) -> Result<Option<String>, ComponentDbError>;

    /// Read key fields of a `reborn_docus` row.
    async fn read_row(
        &self,
        scope: &ComponentDbScope,
        name: &str,
    ) -> Result<Option<ComponentDbRow>, ComponentDbError>;

    /// Insert-or-update a `reborn_docus` row. Always sets
    /// `validation_status='pending'` so the row enters Q1+Q2.
    async fn upsert(
        &self,
        scope: &ComponentDbScope,
        row: ComponentDbUpsert,
    ) -> Result<ComponentDbUpsertResult, ComponentDbError>;

    /// Mark the base-prompt prefix stale for `(scope.user_id, scope.project_id)`.
    /// Wraps `PgBasicPromptStore::mark_stale`. Best-effort — errors logged but
    /// not fatal.
    async fn mark_stale(&self, scope: &ComponentDbScope) -> Result<(), ComponentDbError>;
}

// ---------------------------------------------------------------------------
// Pure-Rust helpers (shared with trait default impls)
// ---------------------------------------------------------------------------

/// Compute lowercase SHA-256 hex of `text`.
pub fn sha256_hex(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

/// Extract a named markdown section.
///
/// Searches for `## <title>` (case-insensitive prefix match on the heading
/// text after stripping leading `#` markers and whitespace). Collects lines
/// until the next `##`-level heading or end of document. Returns the section
/// body (without the heading line itself) with leading/trailing whitespace
/// trimmed.
///
/// Handles both numbered headings (`## 7. LLM-summary`) and plain headings
/// (`## LLM-summary`). The caller may pass just the bare title (without `##`)
/// or a partial match — the function strips `## ` + optional `N. ` prefix
/// before comparing.
pub fn extract_md_section(markdown: &str, title: &str) -> Option<String> {
    let title_lower = title.to_lowercase();
    let mut in_section = false;
    let mut section_lines: Vec<&str> = Vec::new();

    for line in markdown.lines() {
        if let Some(stripped) = line.strip_prefix("## ") {
            // Strip optional numeric prefix like "7. " from heading text.
            let heading_text = stripped
                .splitn(2, ". ")
                .last()
                .unwrap_or(stripped)
                .trim();
            if in_section {
                // Hit the next ## heading — section ends.
                break;
            }
            if heading_text.to_lowercase().contains(&title_lower)
                || title_lower.contains(&heading_text.to_lowercase())
            {
                in_section = true;
            }
        } else if in_section {
            section_lines.push(line);
        }
    }

    if in_section {
        Some(section_lines.join("\n").trim().to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// State (carries the optional backend)
// ---------------------------------------------------------------------------

/// Capability state for `builtin.component_db`. Carries an optional backend
/// injected at composition time.
#[derive(Default, Clone)]
pub struct ComponentDbState {
    backend: Option<Arc<dyn ComponentDbBackend>>,
}

impl ComponentDbState {
    /// Wire in a concrete backend.
    pub fn with_backend(backend: Arc<dyn ComponentDbBackend>) -> Self {
        Self {
            backend: Some(backend),
        }
    }
}

impl std::fmt::Debug for ComponentDbState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentDbState")
            .field("backend_wired", &self.backend.is_some())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

pub(super) async fn dispatch(
    state: &ComponentDbState,
    input: &Value,
) -> Result<Value, FirstPartyCapabilityError> {
    let op = input
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| input_error("missing required field: op"))?;

    // compute_hash and extract_section are pure operations — they don't need
    // the backend (but we still accept a wired backend to allow overriding).
    match op {
        "compute_hash" => {
            let text = input
                .get("text")
                .and_then(Value::as_str)
                .ok_or_else(|| input_error("compute_hash requires: text (str)"))?;
            let hash = sha256_hex(text);
            return Ok(json!({"hash": hash}));
        }
        "extract_section" => {
            let markdown = input
                .get("markdown")
                .and_then(Value::as_str)
                .ok_or_else(|| input_error("extract_section requires: markdown (str)"))?;
            let title = input
                .get("title")
                .and_then(Value::as_str)
                .ok_or_else(|| input_error("extract_section requires: title (str)"))?;
            let result = extract_md_section(markdown, title);
            return Ok(json!({"section": result}));
        }
        _ => {}
    }

    // All remaining ops require a wired backend.
    let backend = state
        .backend
        .as_ref()
        .ok_or(ComponentDbError::NotWired)
        .map_err(db_error)?;

    let scope = parse_scope(input)?;

    match op {
        "read_hash" => {
            let name = require_str(input, "name", "read_hash")?;
            let hash = backend.read_hash(&scope, name).await.map_err(db_error)?;
            Ok(json!({"hash": hash}))
        }
        "read_row" => {
            let name = require_str(input, "name", "read_row")?;
            let row = backend.read_row(&scope, name).await.map_err(db_error)?;
            Ok(match row {
                Some(r) => json!({
                    "found": true,
                    "id": r.id,
                    "name": r.name,
                    "content": r.content,
                    "content_hash": r.content_hash,
                    "validation_status": r.validation_status,
                }),
                None => json!({"found": false}),
            })
        }
        "upsert" => {
            let fields = input
                .get("fields")
                .ok_or_else(|| input_error("upsert requires: fields (object)"))?;
            let upsert_row = ComponentDbUpsert {
                name: require_str_from(fields, "name", "upsert.fields")?.to_string(),
                description: require_str_from(fields, "description", "upsert.fields")?
                    .to_string(),
                content: require_str_from(fields, "content", "upsert.fields")?.to_string(),
                content_hash: require_str_from(fields, "content_hash", "upsert.fields")?
                    .to_string(),
                source: fields
                    .get("source")
                    .and_then(Value::as_str)
                    .unwrap_or("system")
                    .to_string(),
                similarity_parent_id: fields
                    .get("similarity_parent_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                replaces_id: fields
                    .get("replaces_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                consumer_tags: fields
                    .get("consumer_tags")
                    .and_then(Value::as_array)
                    .map(|arr| arr.iter().filter_map(Value::as_str).map(str::to_string).collect())
                    .unwrap_or_else(|| vec!["03:llm".to_string()]),
            };
            let result = backend.upsert(&scope, upsert_row).await.map_err(db_error)?;
            Ok(json!({
                "id": result.id,
                "content_hash": result.content_hash,
                "is_new": result.is_new,
            }))
        }
        "mark_stale" => {
            backend.mark_stale(&scope).await.map_err(db_error)?;
            Ok(json!({"ok": true}))
        }
        unknown => Err(input_error(format!("unknown op: {unknown}"))),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_scope(input: &Value) -> Result<ComponentDbScope, FirstPartyCapabilityError> {
    let scope = input
        .get("scope")
        .ok_or_else(|| input_error("missing required field: scope"))?;
    Ok(ComponentDbScope {
        user_id: require_str_from(scope, "user_id", "scope")?.to_string(),
        project_id: require_str_from(scope, "project_id", "scope")?.to_string(),
    })
}

fn require_str<'a>(
    input: &'a Value,
    field: &'static str,
    op: &'static str,
) -> Result<&'a str, FirstPartyCapabilityError> {
    input
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| input_error(format!("{op} requires: {field} (str)")))
}

fn require_str_from<'a>(
    obj: &'a Value,
    field: &'static str,
    context: &'static str,
) -> Result<&'a str, FirstPartyCapabilityError> {
    obj.get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| input_error(format!("{context}.{field} is required (str)")))
}

fn input_error(msg: impl Into<String>) -> FirstPartyCapabilityError {
    use brassclaw_host_api::RuntimeDispatchErrorKind;
    FirstPartyCapabilityError::with_safe_summary(RuntimeDispatchErrorKind::Client, msg)
}

fn db_error(e: ComponentDbError) -> FirstPartyCapabilityError {
    use brassclaw_host_api::RuntimeDispatchErrorKind;
    FirstPartyCapabilityError::with_safe_summary(
        RuntimeDispatchErrorKind::Backend,
        e.to_string(),
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vector() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        assert_eq!(
            sha256_hex(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // SHA-256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        assert_eq!(
            sha256_hex("hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn extract_section_numbered_heading() {
        let md = "# Doc\n\n## 1. Purpose\n\nPurpose text.\n\n## 7. LLM-summary\n\nSummary line 1.\nSummary line 2.\n\n## 8. Other\n\nOther.\n";
        let section = extract_md_section(md, "LLM-summary").unwrap();
        assert!(section.contains("Summary line 1."));
        assert!(section.contains("Summary line 2."));
        assert!(!section.contains("Other."));
    }

    #[test]
    fn extract_section_plain_heading() {
        let md = "## Background\n\nBg text.\n\n## LLM-summary\n\nCompressed.\n";
        let section = extract_md_section(md, "LLM-summary").unwrap();
        assert_eq!(section, "Compressed.");
    }

    #[test]
    fn extract_section_not_found() {
        let md = "## Intro\n\nSome text.\n";
        assert!(extract_md_section(md, "Missing").is_none());
    }

    #[tokio::test]
    async fn dispatch_compute_hash() {
        let state = ComponentDbState::default();
        let result = dispatch(
            &state,
            &json!({"op": "compute_hash", "text": "hello"}),
        )
        .await
        .unwrap();
        assert_eq!(
            result["hash"].as_str().unwrap(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[tokio::test]
    async fn dispatch_extract_section() {
        let state = ComponentDbState::default();
        let md = "## 7. LLM-summary\n\nCompressed doc.";
        let result = dispatch(
            &state,
            &json!({"op": "extract_section", "markdown": md, "title": "LLM-summary"}),
        )
        .await
        .unwrap();
        assert_eq!(result["section"].as_str().unwrap(), "Compressed doc.");
    }

    #[tokio::test]
    async fn dispatch_read_hash_not_wired() {
        let state = ComponentDbState::default();
        let result = dispatch(
            &state,
            &json!({"op": "read_hash", "scope": {"user_id": "u", "project_id": "p"}, "name": "x"}),
        )
        .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not wired") || err.to_string().contains("NotWired"));
    }
}
