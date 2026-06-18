//! Capability permission storage for v2 capabilities.
//!
//! This module provides persistent storage for capability permission overrides.
//! Permissions follow a resolution hierarchy: override → descriptor default → deny.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_host_api::PermissionMode;

use crate::db::Database;
use crate::error::DatabaseError;

/// Store for capability permission overrides.
///
/// Permissions are stored per-tenant (user_id) and per-capability.
/// When no override exists, the capability's default permission is used.
#[async_trait]
pub trait CapabilityPermissionStore: Send + Sync {
    /// Get the permission override for a specific capability.
    ///
    /// Returns `None` if no override exists (use descriptor default).
    async fn get_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<Option<PermissionMode>, DatabaseError>;

    /// Set a permission override for a specific capability.
    ///
    /// Overwrites any existing override for this tenant/capability pair.
    async fn set_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
        mode: PermissionMode,
    ) -> Result<(), DatabaseError>;

    /// Delete a permission override for a specific capability.
    ///
    /// Returns `true` if an override was deleted, `false` if none existed.
    async fn delete_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<bool, DatabaseError>;

    /// List all permission overrides for a tenant.
    ///
    /// Returns a map of capability_id → permission_mode.
    async fn list_overrides(
        &self,
        tenant_id: &str,
    ) -> Result<HashMap<String, PermissionMode>, DatabaseError>;

    /// Delete all permission overrides for a tenant.
    ///
    /// Returns the number of overrides deleted.
    async fn clear_overrides(&self, tenant_id: &str) -> Result<usize, DatabaseError>;
}

/// In-memory implementation of CapabilityPermissionStore.
///
/// Useful for testing and development. Does not persist across restarts.
pub struct InMemoryPermissionStore {
    // tenant_id → (capability_id → permission_mode)
    overrides: tokio::sync::RwLock<HashMap<String, HashMap<String, PermissionMode>>>,
}

impl InMemoryPermissionStore {
    pub fn new() -> Self {
        Self {
            overrides: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryPermissionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CapabilityPermissionStore for InMemoryPermissionStore {
    async fn get_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<Option<PermissionMode>, DatabaseError> {
        let overrides = self.overrides.read().await;
        Ok(overrides
            .get(tenant_id)
            .and_then(|tenant_overrides| tenant_overrides.get(capability_id))
            .copied())
    }

    async fn set_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
        mode: PermissionMode,
    ) -> Result<(), DatabaseError> {
        let mut overrides = self.overrides.write().await;
        overrides
            .entry(tenant_id.to_string())
            .or_insert_with(HashMap::new)
            .insert(capability_id.to_string(), mode);
        Ok(())
    }

    async fn delete_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<bool, DatabaseError> {
        let mut overrides = self.overrides.write().await;
        Ok(overrides
            .get_mut(tenant_id)
            .and_then(|tenant_overrides| tenant_overrides.remove(capability_id))
            .is_some())
    }

    async fn list_overrides(
        &self,
        tenant_id: &str,
    ) -> Result<HashMap<String, PermissionMode>, DatabaseError> {
        let overrides = self.overrides.read().await;
        Ok(overrides
            .get(tenant_id)
            .cloned()
            .unwrap_or_else(HashMap::new))
    }

    async fn clear_overrides(&self, tenant_id: &str) -> Result<usize, DatabaseError> {
        let mut overrides = self.overrides.write().await;
        Ok(overrides
            .remove(tenant_id)
            .map(|tenant_overrides| tenant_overrides.len())
            .unwrap_or(0))
    }
}

/// Database-backed implementation of CapabilityPermissionStore.
///
/// Stores permission overrides in the `capability_permissions` table.
pub struct DbPermissionStore {
    db: Arc<dyn Database>,
}

impl DbPermissionStore {
    pub fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CapabilityPermissionStore for DbPermissionStore {
    async fn get_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<Option<PermissionMode>, DatabaseError> {
        self.db.get_capability_permission(tenant_id, capability_id).await
    }

    async fn set_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
        mode: PermissionMode,
    ) -> Result<(), DatabaseError> {
        self.db.set_capability_permission(tenant_id, capability_id, mode).await
    }

    async fn delete_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<bool, DatabaseError> {
        self.db.delete_capability_permission(tenant_id, capability_id).await
    }

    async fn list_overrides(
        &self,
        tenant_id: &str,
    ) -> Result<HashMap<String, PermissionMode>, DatabaseError> {
        self.db.list_capability_overrides(tenant_id).await
    }

    async fn clear_overrides(&self, tenant_id: &str) -> Result<usize, DatabaseError> {
        // The Database trait doesn't have clear_overrides, so we need to list and delete each one
        let overrides = self.db.list_capability_overrides(tenant_id).await?;
        let count = overrides.len();
        for capability_id in overrides.keys() {
            self.db.delete_capability_permission(tenant_id, capability_id).await?;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_store_basic_operations() {
        let store = InMemoryPermissionStore::new();
        let tenant = "user123";
        let cap_id = "builtin.read_file";

        // Initially no override
        assert_eq!(
            store.get_permission(tenant, cap_id).await.unwrap(),
            None
        );

        // Set override
        store
            .set_permission(tenant, cap_id, PermissionMode::Ask)
            .await
            .unwrap();
        assert_eq!(
            store.get_permission(tenant, cap_id).await.unwrap(),
            Some(PermissionMode::Ask)
        );

        // Update override
        store
            .set_permission(tenant, cap_id, PermissionMode::Deny)
            .await
            .unwrap();
        assert_eq!(
            store.get_permission(tenant, cap_id).await.unwrap(),
            Some(PermissionMode::Deny)
        );

        // Delete override
        assert!(store.delete_permission(tenant, cap_id).await.unwrap());
        assert_eq!(
            store.get_permission(tenant, cap_id).await.unwrap(),
            None
        );

        // Delete non-existent override
        assert!(!store.delete_permission(tenant, cap_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_in_memory_store_list_overrides() {
        let store = InMemoryPermissionStore::new();
        let tenant = "user123";

        // Initially empty
        assert!(store.list_overrides(tenant).await.unwrap().is_empty());

        // Add multiple overrides
        store
            .set_permission(tenant, "builtin.read_file", PermissionMode::Allow)
            .await
            .unwrap();
        store
            .set_permission(tenant, "builtin.write_file", PermissionMode::Ask)
            .await
            .unwrap();
        store
            .set_permission(tenant, "builtin.shell", PermissionMode::Deny)
            .await
            .unwrap();

        let overrides = store.list_overrides(tenant).await.unwrap();
        assert_eq!(overrides.len(), 3);
        assert_eq!(
            overrides.get("builtin.read_file"),
            Some(&PermissionMode::Allow)
        );
        assert_eq!(
            overrides.get("builtin.write_file"),
            Some(&PermissionMode::Ask)
        );
        assert_eq!(
            overrides.get("builtin.shell"),
            Some(&PermissionMode::Deny)
        );
    }

    #[tokio::test]
    async fn test_in_memory_store_clear_overrides() {
        let store = InMemoryPermissionStore::new();
        let tenant = "user123";

        // Add overrides
        store
            .set_permission(tenant, "builtin.read_file", PermissionMode::Allow)
            .await
            .unwrap();
        store
            .set_permission(tenant, "builtin.write_file", PermissionMode::Ask)
            .await
            .unwrap();

        // Clear all
        let count = store.clear_overrides(tenant).await.unwrap();
        assert_eq!(count, 2);
        assert!(store.list_overrides(tenant).await.unwrap().is_empty());

        // Clear empty tenant
        let count = store.clear_overrides(tenant).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_in_memory_store_tenant_isolation() {
        let store = InMemoryPermissionStore::new();
        let tenant1 = "user123";
        let tenant2 = "user456";
        let cap_id = "builtin.read_file";

        // Set different permissions for different tenants
        store
            .set_permission(tenant1, cap_id, PermissionMode::Allow)
            .await
            .unwrap();
        store
            .set_permission(tenant2, cap_id, PermissionMode::Deny)
            .await
            .unwrap();

        // Verify isolation
        assert_eq!(
            store.get_permission(tenant1, cap_id).await.unwrap(),
            Some(PermissionMode::Allow)
        );
        assert_eq!(
            store.get_permission(tenant2, cap_id).await.unwrap(),
            Some(PermissionMode::Deny)
        );

        // Clear one tenant doesn't affect the other
        store.clear_overrides(tenant1).await.unwrap();
        assert_eq!(
            store.get_permission(tenant1, cap_id).await.unwrap(),
            None
        );
        assert_eq!(
            store.get_permission(tenant2, cap_id).await.unwrap(),
            Some(PermissionMode::Deny)
        );
    }
}

// Made with Bob
