//! `ToolRegistry` — capability surface for Rusty tool definitions.
//!
//! This is the surface that [`RecipeValidator::validate_tool_skill`] checks
//! `tool_name` against.  It holds a snapshot of validated Rusty tool names
//! (class_code = 00, no `05:validator` tag) loaded from `reborn_tools`.
//!
//! The surface is intentionally minimal: we expose only the names needed for
//! validation, not raw DB rows.  No Monty/LLM prompt text is included — tools
//! are Rusty-only.
//!
//! ## Lifetime model
//!
//! A [`ToolRegistry`] is a snapshot that callers build at the start of a
//! validation pass by calling [`ToolRegistryStore::fetch_tool_names`].  It
//! does not auto-refresh; call sites that need up-to-date data should fetch a
//! new snapshot.
//!
//! ## Scope isolation
//!
//! All DB reads filter on the full `(tenant_id, user_id, agent_id,
//! project_id)` scope tuple.  Reads from a different scope return an empty
//! set — see the contract test in `tests/`.

use async_trait::async_trait;
use thiserror::Error;

/// Full `(tenant_id, user_id, agent_id, project_id)` scope tuple.
///
/// All DB reads for tool definitions must filter on the complete tuple so
/// that tools from one scope cannot leak into another scope's validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolScopeKey {
    pub tenant_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub project_id: String,
}

/// In-memory snapshot of validated Rusty tool names for a single scope.
///
/// Carries only the names that [`RecipeValidator::validate_tool_skill`] needs
/// to check whether a ToolSkill's `tool_name` is present in the capability
/// surface.
#[derive(Debug, Clone, Default)]
pub struct ToolRegistry {
    names: Vec<String>,
}

impl ToolRegistry {
    /// Build from a pre-fetched name list.
    pub fn from_names(names: Vec<String>) -> Self {
        Self { names }
    }

    /// Borrow the tool name slice for validator calls.
    ///
    /// Pass this slice directly to `RecipeValidator::validate_tool_skill`.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Returns `true` if `tool_name` is present in the registry.
    pub fn contains(&self, tool_name: &str) -> bool {
        self.names.iter().any(|n| n == tool_name)
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Returns `true` if the registry holds no tool names.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

/// Error type for `ToolRegistryStore` operations.
#[derive(Debug, Error)]
pub enum ToolRegistryError {
    #[error("DB query for tool names failed: {reason}")]
    QueryFailed { reason: String },
}

/// Async port for loading the `ToolRegistry` from a backing store.
///
/// The default implementation is [`brassclaw_engine::capability::DbToolSource`]
/// (behind the `skills-db` feature).  Unit tests can provide in-memory stubs.
#[async_trait]
pub trait ToolRegistryStore: Send + Sync {
    /// Return the names of all **validated** Rusty tools in scope.
    ///
    /// - Only rows with `validation_status = 'validated'` are returned.
    /// - Rows that still carry the `05:validator` consumer tag are excluded
    ///   (the validator tag greys out delivery — §3.5.1).
    /// - The result is filtered to `class_code = 0` (Rusty only).
    ///
    /// An empty `Vec` is a valid result (no validated tools yet).
    async fn fetch_tool_names(
        &self,
        scope: &ToolScopeKey,
    ) -> Result<Vec<String>, ToolRegistryError>;
}
