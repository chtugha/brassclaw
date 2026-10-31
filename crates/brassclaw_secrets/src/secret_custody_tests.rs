//! Actual encrypted memory and filesystem custody owners.
use crate::{
    FilesystemSecretStore, InMemorySecretStore, ScopedSecretsStoreAdapter, SecretCustodySource,
    SecretCustodyState, SecretIdentityClosure, SecretStore, SecretStoreError, SecretsCrypto,
};
use brassclaw_filesystem::{InMemoryBackend, RootFilesystem, ScopedFilesystem};
use brassclaw_host_api::{
    InvocationId, MountAlias, MountGrant, MountPermissions, MountView, ResourceScope, SecretHandle,
    UserId, VirtualPath,
};
use std::sync::Arc;
#[path = "../tests/support/custody_contract.rs"]
mod contract;
fn scope() -> ResourceScope {
    ResourceScope::local_default(UserId::new("custody-owner").unwrap(), InvocationId::new())
        .unwrap()
}
fn crypto() -> Arc<SecretsCrypto> {
    Arc::new(SecretsCrypto::new("actual-custody-native-master-key-2026".into()).unwrap())
}
fn filesystem<F: RootFilesystem + 'static>(
    backend: Arc<F>,
    owner: &ResourceScope,
) -> Arc<dyn SecretStore> {
    // Internal System IDs deliberately contain control characters and are not
    // virtual path components. The test mount is a private storage destination;
    // actual typed caller/body scope remains System and is authenticated below.
    let destination = if owner.is_system() {
        "/secrets/custody-contract-system".to_owned()
    } else {
        format!(
            "/tenants/{}/users/{}/secrets",
            owner.tenant_id, owner.user_id
        )
    };
    let mounts = MountView::new(vec![MountGrant::new(
        MountAlias::new("/secrets").unwrap(),
        VirtualPath::new(destination).unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    Arc::new(FilesystemSecretStore::new(
        Arc::new(ScopedFilesystem::with_fixed_view(backend, mounts)),
        crypto(),
    ))
}
#[tokio::test]
async fn actual_encrypted_memory_custody_preserves_first_values_and_retirement() {
    let store = Arc::new(InMemorySecretStore::new());
    contract::verify(store.clone(), store, scope()).await;
}
#[tokio::test]
async fn actual_legacy_scope_adapters_preserve_first_values_and_retirement() {
    let inner = Arc::new(crate::legacy_store::InMemorySecretsStore::new(crypto()));
    contract::verify(
        Arc::new(ScopedSecretsStoreAdapter::new(inner.clone())),
        Arc::new(ScopedSecretsStoreAdapter::new(inner)),
        scope(),
    )
    .await;
}
#[tokio::test]
async fn actual_independent_filesystem_custody_preserves_first_values_and_retirement() {
    let owner = scope();
    let root = Arc::new(InMemoryBackend::new());
    contract::verify(
        filesystem(root.clone(), &owner),
        filesystem(root, &owner),
        owner,
    )
    .await;
}
#[tokio::test]
async fn actual_internal_system_custody_rehydrates_without_public_scope_grants() {
    let owner = ResourceScope::system();
    let root = Arc::new(InMemoryBackend::new());
    contract::verify(
        filesystem(root.clone(), &owner),
        filesystem(root, &owner),
        owner.clone(),
    )
    .await;
    let memory = Arc::new(InMemorySecretStore::new());
    contract::verify(memory.clone(), memory, owner).await;
}

#[tokio::test]
async fn actual_fs_ordinary_delete_fences_a_new_immutable_original_after_recreation() {
    let backend = Arc::new(InMemoryBackend::new());
    crate::secret_reference_tests::cas_aba::verify_secret_delete_aba(
        backend.clone(),
        backend,
        scope(),
        crypto(),
    )
    .await;
}
