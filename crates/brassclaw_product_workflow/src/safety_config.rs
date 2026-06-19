//! Safety configuration types for WebUI v2.

use serde::{Deserialize, Serialize};

/// Response shape for safety configuration endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConfigResponse {
    pub entries: Vec<SafetyEntry>,
}

/// A single safety rule entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyEntry {
    /// The pattern to match (e.g., ".env", "/.ssh/", "/dev/zero")
    pub pattern: String,
    /// Whether this rule is currently enabled
    pub enabled: bool,
    /// Whether this is a system default (true) or user-added (false)
    pub is_default: bool,
}

/// Request body for updating safety configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSafetyConfigRequest {
    pub entries: Vec<SafetyEntry>,
}

