//! V1 Web Platform State Module Stub
//!
//! This module provides stubs for the deleted V1 web platform state system.

// ============================================================================
// V1 STUBS - TODO: Remove after V2 migration complete
// ============================================================================

use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Stub for deleted V1 PlatformState
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformState {
    pub active: bool,
}

impl PlatformState {
    pub fn new() -> Self {
        Self { active: false }
    }
}

impl Default for PlatformState {
    fn default() -> Self {
        Self::new()
    }
}

/// Stub for deleted V1 WorkspacePool
#[derive(Debug, Clone)]
pub struct WorkspacePool {
    workspaces: Vec<Arc<crate::workspace::Workspace>>,
}

impl WorkspacePool {
    pub fn new() -> Self {
        Self {
            workspaces: Vec::new(),
        }
    }
}

impl Default for WorkspacePool {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// END V1 STUBS
// ============================================================================

// Made with Bob
