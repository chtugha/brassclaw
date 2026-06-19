//! V1 Tools Module Stub
//!
//! This module provides stubs for the deleted V1 tools system.
//! All functionality has been migrated to V2 (brassclaw-reborn).

// ============================================================================
// V1 STUBS - TODO: Remove after V2 migration complete
// ============================================================================

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod builtin;
pub mod permissions;

/// Stub for deleted V1 ToolRegistry
#[derive(Debug, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, String>,
}

impl ToolRegistry {
    /// Create a new empty tool registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool (no-op stub)
    pub fn register(&mut self, _name: String, _description: String) -> Result<()> {
        Ok(())
    }

    /// Get all registered tools
    pub fn tools(&self) -> &HashMap<String, String> {
        &self.tools
    }

    /// Register routine tools (no-op stub)
    /// TODO: Remove after V2 migration complete
    pub fn register_routine_tools(
        &self,
        _store: std::sync::Arc<crate::db::Database>,
        _engine: std::sync::Arc<crate::agent::routine_engine::RoutineEngine>,
    ) {
        // No-op
    }

    /// Set message tool context (no-op stub)
    /// TODO: Remove after V2 migration complete
    pub async fn set_message_tool_context(
        &self,
        _channel: Option<String>,
        _target: Option<String>,
    ) {
        // No-op
    }

    /// List all tool names (stub returns empty list)
    /// TODO: Remove after V2 migration complete
    pub fn list(&self) -> Vec<String> {
        Vec::new()
    }

    /// Get tool definitions visible under a policy (stub returns empty list)
    /// TODO: Remove after V2 migration complete
    pub async fn tool_definitions_visible_under(
        &self,
        _policy: &brassclaw_host_api::EffectiveRuntimePolicy,
    ) -> Vec<brassclaw_llm::ToolDefinition> {
        Vec::new()
    }

    /// Get all tool definitions (stub returns empty list)
    /// TODO: Remove after V2 migration complete
    pub async fn tool_definitions(&self) -> Vec<brassclaw_llm::ToolDefinition> {
        Vec::new()
    }

    /// Get a tool by name (stub returns None)
    /// TODO: Remove after V2 migration complete
    pub fn get(&self, _name: &str) -> Option<Tool> {
        None
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Stub for deleted V1 Tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
}

impl Tool {
    /// Create a new tool stub
    pub fn new(name: String, description: String) -> Self {
        Self { name, description }
    }
}

/// Stub for deleted V1 ToolCall
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    pub args: serde_json::Value,
}

/// Stub for deleted V1 ToolResult
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
}

/// Stub for deleted V1 BuilderConfig
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuilderConfig {
    pub enabled: bool,
}

impl Default for BuilderConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

/// Stub for deleted V1 redact_params function
pub fn redact_params(_params: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({})
}

// ============================================================================
// END V1 STUBS
// ============================================================================

// Made with Bob
