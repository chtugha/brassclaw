//! Safety configuration store trait.
//!
//! The libSQL (`SqliteSafetyConfigStore`) implementation was removed in
//! Phase 6 (libSQL removal). Use `PgSafetyConfigStore` in production.

use async_trait::async_trait;

use crate::safety_config::{SafetyConfigResponse, SafetyEntry};

/// Categories for safety configuration entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyCategory {
    SensitivePaths,
    WorkspaceRules,
    BlockedPaths,
}

impl SafetyCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            SafetyCategory::SensitivePaths => "sensitive_paths",
            SafetyCategory::WorkspaceRules => "workspace_rules",
            SafetyCategory::BlockedPaths => "blocked_paths",
        }
    }
}

/// Trait for storing and retrieving safety configuration.
#[async_trait]
pub trait SafetyConfigStore: Send + Sync {
    /// Get all safety entries for a specific category and user.
    async fn get_config(
        &self,
        user_id: &str,
        category: SafetyCategory,
    ) -> Result<SafetyConfigResponse, Box<dyn std::error::Error + Send + Sync>>;

    /// Update safety configuration for a specific category and user.
    /// This replaces all entries for the category.
    async fn update_config(
        &self,
        user_id: &str,
        category: SafetyCategory,
        entries: Vec<SafetyEntry>,
    ) -> Result<SafetyConfigResponse, Box<dyn std::error::Error + Send + Sync>>;

    /// Initialize default safety rules for a user if they don't exist.
    async fn initialize_defaults(
        &self,
        user_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
