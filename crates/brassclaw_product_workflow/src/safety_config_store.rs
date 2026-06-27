//! Safety configuration database store.
//!
//! This module provides the database layer for storing and retrieving safety
//! configuration entries (sensitive paths, workspace rules, blocked paths).

use async_trait::async_trait;
use libsql::{params, Connection, Database};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::safety_config::{SafetyConfigResponse, SafetyEntry};
use crate::CapabilityPermissionStore;

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

/// LibSQL implementation of SafetyConfigStore.
pub struct SqliteSafetyConfigStore {
    db: Arc<Database>,
    write_lock: Arc<Mutex<()>>,
}

impl SqliteSafetyConfigStore {
    /// Open the store and ensure the required tables exist.
    /// Use this instead of `new()` in production so the first request
    /// does not fail with "no such table".
    pub async fn open(db: Arc<Database>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let store = Self::new(db);
        store.ensure_tables().await?;
        Ok(store)
    }

    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    async fn ensure_tables(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let conn = self.connect().await?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS safety_config (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id TEXT NOT NULL,
                category TEXT NOT NULL,
                pattern TEXT NOT NULL,
                is_enabled BOOLEAN NOT NULL DEFAULT 1,
                is_default BOOLEAN NOT NULL DEFAULT 0,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(user_id, category, pattern)
            );
            CREATE INDEX IF NOT EXISTS idx_safety_config_user_category
                ON safety_config(user_id, category);
            CREATE INDEX IF NOT EXISTS idx_safety_config_enabled
                ON safety_config(is_enabled);
            CREATE TABLE IF NOT EXISTS capability_permissions (
                tenant_id TEXT NOT NULL,
                capability_id TEXT NOT NULL,
                permission_mode TEXT NOT NULL CHECK (permission_mode IN ('allow', 'ask', 'deny')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                PRIMARY KEY (tenant_id, capability_id)
            );
            CREATE INDEX IF NOT EXISTS idx_capability_permissions_tenant
                ON capability_permissions(tenant_id);
            CREATE INDEX IF NOT EXISTS idx_capability_permissions_capability
                ON capability_permissions(capability_id);",
        )
        .await?;
        Ok(())
    }

    /// Get default patterns for each category.
    fn get_default_patterns(category: SafetyCategory) -> Vec<(&'static str, bool)> {
        match category {
            SafetyCategory::SensitivePaths => vec![
                ("*.pem", true),
                ("*.key", true),
                ("*.env", true),
                ("*/.ssh/*", true),
                ("*/.aws/*", true),
                ("*/credentials", true),
                ("*/id_rsa*", true),
            ],
            SafetyCategory::WorkspaceRules => vec![
                ("MEMORY.md", true),
                ("IDENTITY.md", true),
                ("CONTEXT.md", true),
                ("*.brassclaw/*", true),
            ],
            SafetyCategory::BlockedPaths => vec![
                ("/dev/zero", true),
                ("/dev/random", true),
                ("/proc/kcore", true),
                ("/sys/firmware/*", true),
            ],
        }
    }

    async fn connect(&self) -> Result<Connection, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.db.connect()?)
    }
}

#[async_trait]
impl SafetyConfigStore for SqliteSafetyConfigStore {
    async fn get_config(
        &self,
        user_id: &str,
        category: SafetyCategory,
    ) -> Result<SafetyConfigResponse, Box<dyn std::error::Error + Send + Sync>> {
        let conn = self.connect().await?;

        // Check if user has any entries for this category
        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM safety_config WHERE user_id = ?1 AND category = ?2",
                params![user_id, category.as_str()],
            )
            .await?;

        let count: i64 = if let Some(row) = rows.next().await? {
            row.get(0)?
        } else {
            0
        };

        // If no entries exist, initialize defaults inline using the same connection
        if count == 0 {
            let defaults = Self::get_default_patterns(category);
            for (pattern, enabled) in defaults {
                conn.execute(
                    "INSERT OR IGNORE INTO safety_config
                     (user_id, category, pattern, is_enabled, is_default)
                     VALUES (?1, ?2, ?3, ?4, 1)",
                    params![user_id, category.as_str(), pattern, if enabled { 1 } else { 0 }],
                )
                .await?;
            }
        }

        // Fetch all entries for this category
        let mut rows = conn
            .query(
                "SELECT pattern, is_enabled, is_default
                 FROM safety_config
                 WHERE user_id = ?1 AND category = ?2
                 ORDER BY is_default DESC, pattern ASC",
                params![user_id, category.as_str()],
            )
            .await?;

        let mut entries = Vec::new();
        while let Some(row) = rows.next().await? {
            entries.push(SafetyEntry {
                pattern: row.get::<String>(0)?,
                enabled: row.get::<i64>(1)? != 0,
                is_default: row.get::<i64>(2)? != 0,
            });
        }

        Ok(SafetyConfigResponse { entries })
    }

    async fn update_config(
        &self,
        user_id: &str,
        category: SafetyCategory,
        entries: Vec<SafetyEntry>,
    ) -> Result<SafetyConfigResponse, Box<dyn std::error::Error + Send + Sync>> {
        let conn = self.connect().await?;
        
        // Serialize writes
        let _guard = self.write_lock.lock().await;
        
        conn.execute("BEGIN IMMEDIATE", ()).await?;

        let result = async {
            // Delete all non-default entries for this category
            conn.execute(
                "DELETE FROM safety_config WHERE user_id = ?1 AND category = ?2 AND is_default = 0",
                params![user_id, category.as_str()],
            )
            .await?;

            // Update default entries' enabled status and insert new user entries
            for entry in &entries {
                if entry.is_default {
                    // Update existing default entry
                    conn.execute(
                        "UPDATE safety_config 
                         SET is_enabled = ?1, updated_at = CURRENT_TIMESTAMP 
                         WHERE user_id = ?2 AND category = ?3 AND pattern = ?4 AND is_default = 1",
                        params![
                            if entry.enabled { 1 } else { 0 },
                            user_id,
                            category.as_str(),
                            entry.pattern.clone()
                        ],
                    )
                    .await?;
                } else {
                    // Insert new user-defined entry
                    conn.execute(
                        "INSERT INTO safety_config (user_id, category, pattern, is_enabled, is_default) 
                         VALUES (?1, ?2, ?3, ?4, 0)
                         ON CONFLICT(user_id, category, pattern) 
                         DO UPDATE SET is_enabled = excluded.is_enabled, updated_at = CURRENT_TIMESTAMP",
                        params![
                            user_id,
                            category.as_str(),
                            entry.pattern.clone(),
                            if entry.enabled { 1 } else { 0 }
                        ],
                    )
                    .await?;
                }
            }

            Ok::<(), libsql::Error>(())
        }
        .await;

        match result {
            Ok(()) => {
                conn.execute("COMMIT", ()).await?;
                // Return updated configuration
                self.get_config(user_id, category).await
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", ()).await;
                Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
        }
    }

    async fn initialize_defaults(
        &self,
        user_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let conn = self.connect().await?;
        
        let categories = [
            SafetyCategory::SensitivePaths,
            SafetyCategory::WorkspaceRules,
            SafetyCategory::BlockedPaths,
        ];

        for category in &categories {
            let defaults = Self::get_default_patterns(*category);

            for (pattern, enabled) in defaults {
                // Insert default entry, ignore if already exists
                conn.execute(
                    "INSERT OR IGNORE INTO safety_config 
                     (user_id, category, pattern, is_enabled, is_default) 
                     VALUES (?1, ?2, ?3, ?4, 1)",
                    params![user_id, category.as_str(), pattern, if enabled { 1 } else { 0 }],
                )
                .await?;
            }
        }

        Ok(())
    }
}

// Implement CapabilityPermissionStore for SqliteSafetyConfigStore
#[async_trait]
impl CapabilityPermissionStore for SqliteSafetyConfigStore {
    async fn get_capability_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<Option<brassclaw_host_api::PermissionMode>, Box<dyn std::error::Error + Send + Sync>> {
        let conn = self.connect().await?;
        
        let mut rows = conn
            .query(
                "SELECT permission_mode FROM capability_permissions WHERE tenant_id = ?1 AND capability_id = ?2",
                params![tenant_id, capability_id],
            )
            .await?;
        
        if let Some(row) = rows.next().await? {
            let mode_str: String = row.get(0)?;
            let mode = match mode_str.as_str() {
                "allow" => brassclaw_host_api::PermissionMode::Allow,
                "ask" => brassclaw_host_api::PermissionMode::Ask,
                "deny" => brassclaw_host_api::PermissionMode::Deny,
                _ => return Err("Invalid permission mode in database".into()),
            };
            Ok(Some(mode))
        } else {
            Ok(None)
        }
    }

    async fn set_capability_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
        mode: brassclaw_host_api::PermissionMode,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _lock = self.write_lock.lock().await;
        let conn = self.connect().await?;
        
        let mode_str = match mode {
            brassclaw_host_api::PermissionMode::Allow => "allow",
            brassclaw_host_api::PermissionMode::Ask => "ask",
            brassclaw_host_api::PermissionMode::Deny => "deny",
        };
        
        conn.execute(
            "INSERT INTO capability_permissions (tenant_id, capability_id, permission_mode) 
             VALUES (?1, ?2, ?3)
             ON CONFLICT(tenant_id, capability_id) 
             DO UPDATE SET permission_mode = excluded.permission_mode, updated_at = CURRENT_TIMESTAMP",
            params![tenant_id, capability_id, mode_str],
        )
        .await?;
        
        Ok(())
    }

    async fn delete_capability_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let _lock = self.write_lock.lock().await;
        let conn = self.connect().await?;
        
        let rows_affected = conn
            .execute(
                "DELETE FROM capability_permissions WHERE tenant_id = ?1 AND capability_id = ?2",
                params![tenant_id, capability_id],
            )
            .await?;
        
        Ok(rows_affected > 0)
    }

    async fn list_capability_overrides(
        &self,
        tenant_id: &str,
    ) -> Result<std::collections::HashMap<String, brassclaw_host_api::PermissionMode>, Box<dyn std::error::Error + Send + Sync>> {
        let conn = self.connect().await?;
        
        let mut rows = conn
            .query(
                "SELECT capability_id, permission_mode FROM capability_permissions WHERE tenant_id = ?1",
                params![tenant_id],
            )
            .await?;
        
        let mut result = std::collections::HashMap::new();
        while let Some(row) = rows.next().await? {
            let capability_id: String = row.get(0)?;
            let mode_str: String = row.get(1)?;
            let mode = match mode_str.as_str() {
                "allow" => brassclaw_host_api::PermissionMode::Allow,
                "ask" => brassclaw_host_api::PermissionMode::Ask,
                "deny" => brassclaw_host_api::PermissionMode::Deny,
                _ => continue, // Skip invalid entries
            };
            result.insert(capability_id, mode);
        }
        
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_as_str() {
        assert_eq!(SafetyCategory::SensitivePaths.as_str(), "sensitive_paths");
        assert_eq!(SafetyCategory::WorkspaceRules.as_str(), "workspace_rules");
        assert_eq!(SafetyCategory::BlockedPaths.as_str(), "blocked_paths");
    }

    #[test]
    fn test_default_patterns_exist() {
        let sensitive = SqliteSafetyConfigStore::get_default_patterns(SafetyCategory::SensitivePaths);
        assert!(!sensitive.is_empty());
        assert!(sensitive.iter().any(|(p, _)| *p == "*.env"));

        let workspace = SqliteSafetyConfigStore::get_default_patterns(SafetyCategory::WorkspaceRules);
        assert!(!workspace.is_empty());
        assert!(workspace.iter().any(|(p, _)| *p == "MEMORY.md"));

        let blocked = SqliteSafetyConfigStore::get_default_patterns(SafetyCategory::BlockedPaths);
        assert!(!blocked.is_empty());
        assert!(blocked.iter().any(|(p, _)| *p == "/dev/zero"));
    }
}

// Made with Bob
