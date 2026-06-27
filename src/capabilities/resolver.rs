//! Permission resolution for v2 capabilities.
//!
//! Implements the permission resolution hierarchy:
//! 1. Check for tenant-specific override in storage
//! 2. Fall back to capability descriptor default
//! 3. Fail-closed with Deny if capability not found

use std::sync::Arc;

use brassclaw_host_api::{CapabilityDescriptor, CapabilityId, PermissionMode};

use super::permissions::CapabilityPermissionStore;

/// Resolves effective permissions for capabilities.
///
/// Resolution follows this hierarchy:
/// 1. **Override**: Tenant-specific permission from storage
/// 2. **Default**: Capability descriptor's default_permission
/// 3. **Deny**: Fail-closed if capability not found
pub struct PermissionResolver {
    store: Arc<dyn CapabilityPermissionStore>,
    descriptors: Arc<tokio::sync::RwLock<Vec<CapabilityDescriptor>>>,
}

impl PermissionResolver {
    /// Create a new permission resolver.
    ///
    /// # Arguments
    /// * `store` - Permission override storage
    /// * `descriptors` - Registered capability descriptors
    pub fn new(
        store: Arc<dyn CapabilityPermissionStore>,
        descriptors: Vec<CapabilityDescriptor>,
    ) -> Self {
        Self {
            store,
            descriptors: Arc::new(tokio::sync::RwLock::new(descriptors)),
        }
    }

    /// Resolve the effective permission for a capability.
    ///
    /// Resolution order:
    /// 1. Check storage for tenant override
    /// 2. Use descriptor default if no override
    /// 3. Return Deny if capability not found (fail-closed)
    pub async fn resolve_permission(&self, tenant_id: &str, capability_id: &str) -> PermissionMode {
        // 1. Check for override
        if let Ok(Some(override_mode)) = self.store.get_permission(tenant_id, capability_id).await {
            return override_mode;
        }

        // 2. Fall back to descriptor default
        let descriptors = self.descriptors.read().await;
        if let Some(descriptor) = descriptors.iter().find(|d| d.id.as_str() == capability_id) {
            return descriptor.default_permission;
        }

        // 3. Fail-closed default
        PermissionMode::Deny
    }

    /// Get the descriptor for a capability.
    pub async fn get_descriptor(&self, capability_id: &str) -> Option<CapabilityDescriptor> {
        let descriptors = self.descriptors.read().await;
        descriptors
            .iter()
            .find(|d| d.id.as_str() == capability_id)
            .cloned()
    }

    /// List all registered capability descriptors.
    pub async fn list_descriptors(&self) -> Vec<CapabilityDescriptor> {
        self.descriptors.read().await.clone()
    }

    /// Register additional capability descriptors.
    ///
    /// This is used to dynamically add capabilities from extensions.
    pub async fn register_descriptors(&self, new_descriptors: Vec<CapabilityDescriptor>) {
        let mut descriptors = self.descriptors.write().await;
        descriptors.extend(new_descriptors);
    }

    /// Unregister capability descriptors by provider.
    ///
    /// This is used when uninstalling extensions.
    pub async fn unregister_provider(&self, provider_id: &str) -> usize {
        let mut descriptors = self.descriptors.write().await;
        let before = descriptors.len();
        descriptors.retain(|d| d.provider.as_str() != provider_id);
        before - descriptors.len()
    }

    /// Check if a capability is registered.
    pub async fn is_registered(&self, capability_id: &str) -> bool {
        let descriptors = self.descriptors.read().await;
        descriptors.iter().any(|d| d.id.as_str() == capability_id)
    }

    /// Get all capability IDs for a specific provider.
    pub async fn list_provider_capabilities(&self, provider_id: &str) -> Vec<CapabilityId> {
        let descriptors = self.descriptors.read().await;
        descriptors
            .iter()
            .filter(|d| d.provider.as_str() == provider_id)
            .map(|d| d.id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_host_api::{EffectKind, ExtensionId, RuntimeKind, TrustClass};
    use serde_json::json;

    use crate::capabilities::permissions::InMemoryPermissionStore;

    fn make_test_descriptor(
        id: &str,
        provider: &str,
        default_permission: PermissionMode,
    ) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new(id).unwrap(),
            provider: ExtensionId::new(provider).unwrap(),
            runtime: RuntimeKind::FirstParty,
            trust_ceiling: TrustClass::Sandbox,
            description: format!("Test capability {}", id),
            parameters_schema: json!({}),
            effects: vec![EffectKind::ReadFilesystem],
            default_permission,
            runtime_credentials: Vec::new(),
            resource_profile: None,
        }
    }

    #[tokio::test]
    async fn test_resolve_with_override() {
        let store = Arc::new(InMemoryPermissionStore::new());
        let descriptors = vec![make_test_descriptor(
            "test.read",
            "test",
            PermissionMode::Ask,
        )];
        let resolver = PermissionResolver::new(store.clone(), descriptors);

        let tenant = "user123";
        let cap_id = "test.read";

        // Without override, uses descriptor default
        assert_eq!(
            resolver.resolve_permission(tenant, cap_id).await,
            PermissionMode::Ask
        );

        // Set override
        store
            .set_permission(tenant, cap_id, PermissionMode::Allow)
            .await
            .unwrap();

        // With override, uses override
        assert_eq!(
            resolver.resolve_permission(tenant, cap_id).await,
            PermissionMode::Allow
        );
    }

    #[tokio::test]
    async fn test_resolve_unknown_capability() {
        let store = Arc::new(InMemoryPermissionStore::new());
        let descriptors = vec![];
        let resolver = PermissionResolver::new(store, descriptors);

        // Unknown capability fails closed with Deny
        assert_eq!(
            resolver
                .resolve_permission("user123", "unknown.capability")
                .await,
            PermissionMode::Deny
        );
    }

    #[tokio::test]
    async fn test_resolve_descriptor_defaults() {
        let store = Arc::new(InMemoryPermissionStore::new());
        let descriptors = vec![
            make_test_descriptor("test.allow", "test", PermissionMode::Allow),
            make_test_descriptor("test.ask", "test", PermissionMode::Ask),
            make_test_descriptor("test.deny", "test", PermissionMode::Deny),
        ];
        let resolver = PermissionResolver::new(store, descriptors);

        let tenant = "user123";

        assert_eq!(
            resolver.resolve_permission(tenant, "test.allow").await,
            PermissionMode::Allow
        );
        assert_eq!(
            resolver.resolve_permission(tenant, "test.ask").await,
            PermissionMode::Ask
        );
        assert_eq!(
            resolver.resolve_permission(tenant, "test.deny").await,
            PermissionMode::Deny
        );
    }

    #[tokio::test]
    async fn test_register_and_unregister() {
        let store = Arc::new(InMemoryPermissionStore::new());
        let initial = vec![make_test_descriptor(
            "builtin.read",
            "builtin",
            PermissionMode::Allow,
        )];
        let resolver = PermissionResolver::new(store, initial);

        // Initially has 1 descriptor
        assert_eq!(resolver.list_descriptors().await.len(), 1);
        assert!(resolver.is_registered("builtin.read").await);

        // Register extension capabilities
        let extension_caps = vec![
            make_test_descriptor("ext.tool1", "extension", PermissionMode::Ask),
            make_test_descriptor("ext.tool2", "extension", PermissionMode::Ask),
        ];
        resolver.register_descriptors(extension_caps).await;

        assert_eq!(resolver.list_descriptors().await.len(), 3);
        assert!(resolver.is_registered("ext.tool1").await);
        assert!(resolver.is_registered("ext.tool2").await);

        // Unregister extension
        let removed = resolver.unregister_provider("extension").await;
        assert_eq!(removed, 2);
        assert_eq!(resolver.list_descriptors().await.len(), 1);
        assert!(!resolver.is_registered("ext.tool1").await);
        assert!(!resolver.is_registered("ext.tool2").await);
        assert!(resolver.is_registered("builtin.read").await);
    }

    #[tokio::test]
    async fn test_list_provider_capabilities() {
        let store = Arc::new(InMemoryPermissionStore::new());
        let descriptors = vec![
            make_test_descriptor("builtin.read", "builtin", PermissionMode::Allow),
            make_test_descriptor("builtin.write", "builtin", PermissionMode::Ask),
            make_test_descriptor("ext.tool", "extension", PermissionMode::Ask),
        ];
        let resolver = PermissionResolver::new(store, descriptors);

        let builtin_caps = resolver.list_provider_capabilities("builtin").await;
        assert_eq!(builtin_caps.len(), 2);
        assert!(builtin_caps.iter().any(|id| id.as_str() == "builtin.read"));
        assert!(builtin_caps.iter().any(|id| id.as_str() == "builtin.write"));

        let ext_caps = resolver.list_provider_capabilities("extension").await;
        assert_eq!(ext_caps.len(), 1);
        assert_eq!(ext_caps[0].as_str(), "ext.tool");
    }

    #[tokio::test]
    async fn test_tenant_isolation() {
        let store = Arc::new(InMemoryPermissionStore::new());
        let descriptors = vec![make_test_descriptor(
            "test.read",
            "test",
            PermissionMode::Ask,
        )];
        let resolver = PermissionResolver::new(store.clone(), descriptors);

        let tenant1 = "user123";
        let tenant2 = "user456";
        let cap_id = "test.read";

        // Set different overrides for different tenants
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
            resolver.resolve_permission(tenant1, cap_id).await,
            PermissionMode::Allow
        );
        assert_eq!(
            resolver.resolve_permission(tenant2, cap_id).await,
            PermissionMode::Deny
        );

        // Tenant without override uses default
        assert_eq!(
            resolver.resolve_permission("user789", cap_id).await,
            PermissionMode::Ask
        );
    }
}
