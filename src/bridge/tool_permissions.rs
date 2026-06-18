//! Stub module for removed tool permissions in bridge layer.
//!
//! Provides compatibility types for v1 legacy code. All permission checks
//! are bypassed (tools are always allowed).

use std::collections::HashMap;
use crate::tools::permissions::PermissionState;

/// Stub: Tool permission resolution (always returns AlwaysAllow).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ToolPermissionResolution {
    pub(crate) effective: PermissionState,
    pub(crate) explicit: Option<PermissionState>,
}

/// Stub: Tool permission snapshot (no permissions enforced).
#[derive(Clone, Default)]
pub(crate) struct ToolPermissionSnapshot {
    _overrides: HashMap<String, PermissionState>,
}

impl ToolPermissionSnapshot {
    /// Stub: Load permissions (returns empty snapshot).
    pub(crate) async fn load(_tools: &crate::tools::ToolRegistry, _user_id: &str) -> Self {
        Self::default()
    }

    /// Stub: Resolve permission (always returns AlwaysAllow).
    pub(crate) fn resolve_permission(&self, _tool_name: &str) -> ToolPermissionResolution {
        ToolPermissionResolution {
            effective: PermissionState::AlwaysAllow,
            explicit: None,
        }
    }
}

// Made with Bob
