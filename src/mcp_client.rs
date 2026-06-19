//! V1 MCP Client Module Stub
//!
//! This module provides stubs for the deleted V1 MCP client system.
//! All functionality has been migrated to V2 (brassclaw-reborn).

// ============================================================================
// V1 STUBS - TODO: Remove after V2 migration complete
// ============================================================================

pub mod config;

use serde::{Deserialize, Serialize};

/// Stub for deleted V1 McpClient
#[derive(Debug, Clone)]
pub struct McpClient {
    connected: bool,
}

impl McpClient {
    /// Create a new MCP client stub
    pub fn new() -> Self {
        Self { connected: false }
    }

    /// Check if connected (always returns false)
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Get connection status
    pub fn status(&self) -> McpStatus {
        McpStatus {
            connected: false,
            server_version: None,
            capabilities: vec![],
        }
    }
}

impl Default for McpClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Stub for MCP connection status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpStatus {
    pub connected: bool,
    pub server_version: Option<String>,
    pub capabilities: Vec<String>,
}

/// Stub for MCP server info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    pub version: String,
}

// ============================================================================
// END V1 STUBS
// ============================================================================

