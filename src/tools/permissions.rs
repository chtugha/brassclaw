//! Stub module for removed tool permissions system.
//!
//! This module provides no-op implementations to maintain compilation
//! compatibility with v1 legacy code. All permission checks are bypassed
//! (tools are always allowed). The v2 Reborn architecture uses capability
//! grants and approval gates instead.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Stub: All tools are always allowed in v1 legacy code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PermissionState {
    AlwaysAllow,
    AskEachTime,
    Disabled,
}

/// Stub: Empty admin tool policy (no tools are restricted).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdminToolPolicy {
    pub disabled_tools: Vec<String>,
}

/// Stub: Cache for admin tool policy (always returns no restrictions).
pub type AdminToolPolicyCache = Arc<RwLock<AdminToolPolicyState>>;

/// Stub: Admin tool policy state.
#[derive(Debug, Clone)]
pub enum AdminToolPolicyState {
    Missing,
    Loaded(AdminToolPolicy),
    FailClosed,
}

/// Stub: Always returns AlwaysAllow (no permission checks).
pub fn effective_permission(
    _tool_name: &str,
    _overrides: &HashMap<String, PermissionState>,
) -> PermissionState {
    PermissionState::AlwaysAllow
}

/// Stub: Returns None (no seeded defaults).
pub fn seeded_default_permission(_tool_name: &str) -> Option<PermissionState> {
    None
}

/// Stub: Always returns true (all tool names are valid).
pub fn is_valid_admin_tool_name(_name: &str) -> bool {
    true
}

/// Stub: Returns empty policy (no tools disabled).
pub async fn load_cached_admin_tool_policy(
    _db: &dyn crate::db::Database,
    _cache: &AdminToolPolicyCache,
) -> AdminToolPolicyState {
    AdminToolPolicyState::Missing
}

/// Stub: Returns all tools unfiltered (no admin restrictions).
pub fn filter_admin_disabled_tools(
    tools: Vec<crate::tools::Tool>,
    _multi_tenant: bool,
    _is_admin: bool,
    _user_id: &str,
    _policy_state: AdminToolPolicyState,
) -> Vec<crate::tools::Tool> {
    tools
}

/// Stub: Parses admin tool policy (returns empty policy).
pub fn parse_admin_tool_policy(
    _value: serde_json::Value,
    _context: &str,
) -> Result<AdminToolPolicy, String> {
    Ok(AdminToolPolicy {
        disabled_tools: Vec::new(),
    })
}

/// Stub: Validates admin tool policy (always succeeds).
pub fn validate_admin_tool_policy(_policy: &AdminToolPolicy) -> Result<(), String> {
    Ok(())
}

/// Reserved user ID for admin settings (moved to crate::tenant).
pub const ADMIN_SETTINGS_USER_ID: &str = "__admin__";

/// Settings key for admin tool policy.
pub const ADMIN_TOOL_POLICY_KEY: &str = "admin_tool_policy";

/// Stub: Reason for locked tool permission (always returns None).
pub const TOOL_PERMISSION_LOCKED_REASON: Option<&str> = None;

/// Stub: Check if tool permission is locked (always returns false).
pub fn tool_permission_locked(_tool_name: &str) -> bool {
    false
}

// Made with Bob
