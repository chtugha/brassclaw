//! LibSQL implementation of CapabilityPermissionStore.

use std::collections::HashMap;

use async_trait::async_trait;
use brassclaw_host_api::PermissionMode;

use crate::db::{CapabilityPermissionStore, DatabaseError};
use crate::db::libsql::LibSqlBackend;

#[async_trait]
impl CapabilityPermissionStore for LibSqlBackend {
    async fn get_capability_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<Option<PermissionMode>, DatabaseError> {
        let conn = self.pool.get().await.map_err(|e| {
            DatabaseError::Pool(format!("failed to get connection: {}", e))
        })?;

        let row = conn
            .query(
                "SELECT permission_mode FROM capability_permissions WHERE tenant_id = ? AND capability_id = ?",
                libsql::params![tenant_id, capability_id],
            )
            .await
            .map_err(|e| DatabaseError::Query(format!("get_capability_permission: {}", e)))?
            .next()
            .await
            .map_err(|e| DatabaseError::Query(format!("get_capability_permission next: {}", e)))?;

        match row {
            Some(row) => {
                let mode_str: String = row.get(0).map_err(|e| {
                    DatabaseError::Query(format!("get permission_mode: {}", e))
                })?;
                let mode = match mode_str.as_str() {
                    "allow" => PermissionMode::Allow,
                    "ask" => PermissionMode::Ask,
                    "deny" => PermissionMode::Deny,
                    _ => return Err(DatabaseError::Query(format!("invalid permission_mode: {}", mode_str))),
                };
                Ok(Some(mode))
            }
            None => Ok(None),
        }
    }

    async fn set_capability_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
        mode: PermissionMode,
    ) -> Result<(), DatabaseError> {
        let conn = self.pool.get().await.map_err(|e| {
            DatabaseError::Pool(format!("failed to get connection: {}", e))
        })?;

        let mode_str = match mode {
            PermissionMode::Allow => "allow",
            PermissionMode::Ask => "ask",
            PermissionMode::Deny => "deny",
        };

        conn.execute(
            "INSERT INTO capability_permissions (tenant_id, capability_id, permission_mode, updated_at)
             VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT (tenant_id, capability_id)
             DO UPDATE SET permission_mode = excluded.permission_mode, updated_at = excluded.updated_at",
            libsql::params![tenant_id, capability_id, mode_str],
        )
        .await
        .map_err(|e| DatabaseError::Query(format!("set_capability_permission: {}", e)))?;

        Ok(())
    }

    async fn delete_capability_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<bool, DatabaseError> {
        let conn = self.pool.get().await.map_err(|e| {
            DatabaseError::Pool(format!("failed to get connection: {}", e))
        })?;

        let rows_affected = conn
            .execute(
                "DELETE FROM capability_permissions WHERE tenant_id = ? AND capability_id = ?",
                libsql::params![tenant_id, capability_id],
            )
            .await
            .map_err(|e| DatabaseError::Query(format!("delete_capability_permission: {}", e)))?;

        Ok(rows_affected > 0)
    }

    async fn list_capability_overrides(
        &self,
        tenant_id: &str,
    ) -> Result<HashMap<String, PermissionMode>, DatabaseError> {
        let conn = self.pool.get().await.map_err(|e| {
            DatabaseError::Pool(format!("failed to get connection: {}", e))
        })?;

        let mut rows = conn
            .query(
                "SELECT capability_id, permission_mode FROM capability_permissions WHERE tenant_id = ?",
                libsql::params![tenant_id],
            )
            .await
            .map_err(|e| DatabaseError::Query(format!("list_capability_overrides: {}", e)))?;

        let mut overrides = HashMap::new();
        while let Some(row) = rows.next().await.map_err(|e| {
            DatabaseError::Query(format!("list_capability_overrides next: {}", e))
        })? {
            let capability_id: String = row.get(0).map_err(|e| {
                DatabaseError::Query(format!("get capability_id: {}", e))
            })?;
            let mode_str: String = row.get(1).map_err(|e| {
                DatabaseError::Query(format!("get permission_mode: {}", e))
            })?;
            let mode = match mode_str.as_str() {
                "allow" => PermissionMode::Allow,
                "ask" => PermissionMode::Ask,
                "deny" => PermissionMode::Deny,
                _ => continue, // Skip invalid modes
            };
            overrides.insert(capability_id, mode);
        }

        Ok(overrides)
    }
}

// Made with Bob
