//! V1 MCP Client Config Module Stub
//!
//! This module provides stubs for the deleted V1 MCP client configuration.

// ============================================================================
// V1 STUBS - TODO: Remove after V2 migration complete
// ============================================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Stub for deleted V1 McpConfig
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub enabled: bool,
    pub server_url: Option<String>,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: None,
        }
    }
}

/// Stub for deleted V1 McpServerConfig
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub url: String,
}

/// Stub for deleted V1 load_mcp_servers function
pub fn load_mcp_servers() -> HashMap<String, McpServerConfig> {
    HashMap::new()
}

// ============================================================================
// END V1 STUBS
// ============================================================================

