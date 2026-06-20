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

// V1 submodules - minimal stubs for compatibility
pub mod builtin {
    //! V1 builtin tools stub module
    
    pub mod path_utils {
        //! Path utilities stub
        use crate::error::Result;
        
        /// Stub for normalize_workspace_path function
        pub fn normalize_workspace_path(path: &str) -> String {
            path.to_string()
        }
        
        /// Stub for validate_path function
        pub fn validate_path(_path: &str) -> Result<()> {
            Ok(())
        }
    }
    
    /// Stub for image_api_endpoint_url function
    pub fn image_api_endpoint_url() -> String {
        String::new()
    }
    
    /// Stub for media_type_from_path function
    pub fn media_type_from_path(_path: &str) -> Option<String> {
        None
    }
}

pub mod permissions {
    //! V1 permissions stub module
    use serde::{Deserialize, Serialize};
    
    /// Stub for V1 PermissionState
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum PermissionState {
        Allowed,
        Denied,
        RequiresApproval,
    }
    
    impl Default for PermissionState {
        fn default() -> Self {
            Self::RequiresApproval
        }
    }
    
    /// Stub for V1 AdminToolPolicy
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum AdminToolPolicy {
        AllowAll,
        DenyAll,
        RequireApproval,
    }
    
    impl Default for AdminToolPolicy {
        fn default() -> Self {
            Self::RequireApproval
        }
    }
}

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
        _store: std::sync::Arc<dyn crate::db::Database>,
        _engine: std::sync::Arc<dyn std::any::Any + Send + Sync>,
    ) {
        // No-op - use Any trait to avoid circular dependency
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
    pub async fn get(&self, _name: &str) -> Option<ToolStub> {
        None
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Stub for deleted V1 ApprovalRequirement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalRequirement {
    Never,
    UnlessAutoApproved,
    Always,
}

/// Trait for V1 Tool compatibility
/// TODO: Remove after V2 migration complete
pub trait Tool {
    fn sensitive_params(&self) -> &[&str];
}

/// Stub for deleted V1 Tool struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStub {
    pub name: String,
    pub description: String,
}

impl ToolStub {
    /// Create a new tool stub
    pub fn new(name: String, description: String) -> Self {
        Self { name, description }
    }

    /// Check if tool requires approval (stub always returns Never)
    /// TODO: Remove after V2 migration complete
    pub fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
        ApprovalRequirement::Never
    }

    /// Get sensitive parameter names (stub returns empty slice)
    /// TODO: Remove after V2 migration complete
    pub fn sensitive_params(&self) -> &[&str] {
        &[]
    }
}

impl Tool for ToolStub {
    fn sensitive_params(&self) -> &[&str] {
        ToolStub::sensitive_params(self)
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

