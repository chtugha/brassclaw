//! V1 Tool Permissions Module Stub
//!
//! This module provides stubs for the deleted V1 tool permissions system.

// ============================================================================
// V1 STUBS - TODO: Remove after V2 migration complete
// ============================================================================

use serde::{Deserialize, Serialize};

/// Stub for deleted V1 ToolPermission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPermission {
    pub tool_name: String,
    pub allowed: bool,
}

/// Stub for deleted V1 PermissionSet
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionSet {
    pub permissions: Vec<ToolPermission>,
}

impl PermissionSet {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Stub for deleted V1 PermissionState
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionState {
    pub allowed: bool,
}

/// Stub for deleted V1 AdminToolPolicy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminToolPolicy {
    pub policy: String,
}

impl Default for AdminToolPolicy {
    fn default() -> Self {
        Self {
            policy: "deny".to_string(),
        }
    }
}

/// Stub for deleted V1 seeded_default_permission function
pub fn seeded_default_permission() -> PermissionSet {
    PermissionSet::new()
}

// ============================================================================
// END V1 STUBS
// ============================================================================

// Made with Bob
