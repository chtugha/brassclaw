//! Actual encrypted memory/filesystem owners for the new setup primitives.
use crate::legacy_store::InMemorySecretsStore;
use crate::{
    CreateSecretParams, FilesystemSecretStore, InMemorySecretStore, ScopedSecretsStoreAdapter,
    SecretCustodySource, SecretLease, SecretStore, SecretStoreError, SecretsCrypto, SecretsStore,
};
use brassclaw_filesystem::{InMemoryBackend, ScopedFilesystem};
use brassclaw_host_api::{InvocationId, ResourceScope, SecretHandle, UserId};
use std::sync::Arc;

#[path = "../tests/support/reference_contract.rs"]
mod contract;

fn scope() -> ResourceScope {
    ResourceScope::local_default(
        UserId::new("secret-reference-owner").unwrap(),
        InvocationId::new(),
    )
    .unwrap()
}
fn crypto() -> Arc<SecretsCrypto> {
    Arc::new(SecretsCrypto::new("native-secret-reference-master-key-2026".into()).unwrap())
}
fn filesystem(
    scope: &ResourceScope,
    backend: Arc<InMemoryBackend>,
) -> Arc<ScopedFilesystem<InMemoryBackend>> {
    use brassclaw_host_api::{MountAlias, MountGrant, MountPermissions, MountView, VirtualPath};
    Arc::new(ScopedFilesystem::with_fixed_view(
        backend,
        MountView::new(vec![MountGrant::new(
            MountAlias::new("/secrets").unwrap(),
            VirtualPath::new(format!(
                "/tenants/{}/users/{}/secrets",
                scope.tenant_id, scope.user_id
            ))
            .unwrap(),
            MountPermissions::read_write_list_delete(),
        )])
        .unwrap(),
    ))
}

#[tokio::test]
async fn encrypted_memory_reference_cas_uses_one_real_scope_owner() {
    let owner = scope();
    let memory = Arc::new(InMemorySecretStore::new());
    contract::verify_reference_contract(memory.clone(), memory, owner).await;
}

#[tokio::test]
async fn encrypted_memory_conditional_lease_uses_selected_generation() {
    let memory = Arc::new(InMemorySecretStore::new());
    contract::verify_selected_lease_contract(memory.clone(), memory, scope()).await;
}

#[tokio::test]
async fn independent_adapters_conditional_lease_uses_actual_owner_selection() {
    let inner = Arc::new(InMemorySecretsStore::new(crypto()));
    let a = Arc::new(ScopedSecretsStoreAdapter::new(inner.clone()));
    let b = Arc::new(ScopedSecretsStoreAdapter::new(inner));
    contract::verify_selected_lease_contract(a, b, scope()).await;
}

#[tokio::test]
async fn independent_filesystem_conditional_lease_uses_same_authenticated_snapshot() {
    let owner = scope();
    let backend = Arc::new(InMemoryBackend::new());
    let a = Arc::new(FilesystemSecretStore::new(
        filesystem(&owner, backend.clone()),
        crypto(),
    ));
    let b = Arc::new(FilesystemSecretStore::new(
        filesystem(&owner, backend),
        crypto(),
    ));
    contract::verify_selected_lease_contract(a, b, owner).await;
}

#[tokio::test]
async fn independent_scope_adapters_share_actual_native_atomic_reference_cas() {
    let inner = Arc::new(InMemorySecretsStore::new(crypto()));
    let a = Arc::new(ScopedSecretsStoreAdapter::new(inner.clone()));
    let b = Arc::new(ScopedSecretsStoreAdapter::new(inner));
    contract::verify_reference_contract(a, b, scope()).await;
}

#[tokio::test]
async fn independent_filesystem_secret_owners_fence_actual_record_versions() {
    let owner = scope();
    let backend = Arc::new(InMemoryBackend::new());
    let a = Arc::new(FilesystemSecretStore::new(
        filesystem(&owner, backend.clone()),
        crypto(),
    ));
    let b = Arc::new(FilesystemSecretStore::new(
        filesystem(&owner, backend.clone()),
        crypto(),
    ));
    contract::verify_reference_contract(a, b, owner.clone()).await;
    let wrong = FilesystemSecretStore::new(
        filesystem(&owner, backend),
        Arc::new(SecretsCrypto::new("wrong-secret-reference-master-key-2026".into()).unwrap()),
    );
    let handle = SecretHandle::new("generation-reference-probe").unwrap();
    assert!(wrong.record_reference(&owner, &handle).await.is_err());
    assert!(
        wrong
            .lease_once_if_record_reference(&owner, &handle, "mismatched-generation")
            .await
            .is_err(),
        "a stale reference must not hide unauthenticated encrypted state"
    );
    assert!(
        wrong
            .matches(&owner, &handle, &"candidate".into())
            .await
            .is_err()
    );
    assert!(
        wrong
            .put_if_record_reference(owner, handle, "replacement".into(), None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn material_comparison_never_grants_use_of_an_expired_secret() {
    let inner = Arc::new(InMemorySecretsStore::new(crypto()));
    let scope = scope();
    let handle = SecretHandle::new("expired-reference-probe").unwrap();
    inner
        .create(
            &crate::scoped_legacy_user_id(&scope),
            CreateSecretParams::new(handle.to_string(), "expired-original")
                .with_expiry(chrono::Utc::now() - chrono::Duration::seconds(1)),
        )
        .await
        .unwrap();
    let store = ScopedSecretsStoreAdapter::new(inner);
    assert!(
        store
            .record_reference(&scope, &handle)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .matches(&scope, &handle, &"expired-original".into())
            .await
            .unwrap()
    );
    assert_eq!(
        store.lease_once(&scope, &handle).await.unwrap_err(),
        SecretStoreError::SecretExpired
    );
}

#[path = "../tests/support/cas_aba.rs"]
pub(super) mod cas_aba;

#[tokio::test]
async fn filesystem_conditional_lease_does_not_select_replacement_during_write() {
    let backend = Arc::new(InMemoryBackend::new());
    cas_aba::verify_conditional_lease_issuance(backend.clone(), backend, scope(), crypto()).await;
}
#[tokio::test]
async fn filesystem_secret_cas_fences_delete_recreate_between_read_and_write() {
    let backend = Arc::new(InMemoryBackend::new());
    cas_aba::verify_secret_aba(backend.clone(), backend, scope(), crypto()).await;
}

#[tokio::test]
async fn filesystem_lease_consume_fences_actual_revoked_body_with_reused_version() {
    let backend = Arc::new(InMemoryBackend::new());
    cas_aba::verify_lease_consumption_aba(backend.clone(), backend, scope(), crypto()).await;
}
