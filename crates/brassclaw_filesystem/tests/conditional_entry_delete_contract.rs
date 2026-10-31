use brassclaw_filesystem::*;
use brassclaw_host_api::*;
use std::sync::Arc;
#[path = "support/conditional_entry_delete_contract.rs"]
mod contract;

#[tokio::test]
async fn native_memory_selected_entry_delete_preserves_replacements_and_directories() {
    let backend = InMemoryBackend::new();
    contract::verify(&backend, &backend).await;
}
#[tokio::test]
async fn selected_entry_delete_checks_scoped_permission_and_composite_capability() {
    let backend = Arc::new(InMemoryBackend::new());
    let target = VirtualPath::new("/tenants/delete-owner/value").unwrap();
    let version = backend
        .put(
            &target,
            Entry::bytes(b"owned".to_vec()),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
    let checksum = EntryBodyChecksum::for_body(b"owned");
    let scope =
        ResourceScope::local_default(UserId::new("delete-owner").unwrap(), InvocationId::new())
            .unwrap();
    let alias = ScopedPath::new("/resources/value").unwrap();
    let view = |permissions| {
        MountView::new(vec![MountGrant::new(
            MountAlias::new("/resources").unwrap(),
            VirtualPath::new("/tenants/delete-owner").unwrap(),
            permissions,
        )])
        .unwrap()
    };
    let readonly =
        ScopedFilesystem::with_fixed_view(backend.clone(), view(MountPermissions::read_only()));
    assert!(
        readonly
            .delete_if_snapshot(&scope, &alias, version, checksum)
            .await
            .is_err()
    );
    assert!(backend.get(&target).await.unwrap().is_some());
    assert!(
        readonly
            .delete_if_snapshot(
                &scope,
                &ScopedPath::new("/memory/value").unwrap(),
                version,
                checksum
            )
            .await
            .is_err()
    );
    assert!(backend.get(&target).await.unwrap().is_some());
    let mut composite = CompositeRootFilesystem::new();
    composite
        .mount(
            MountDescriptor {
                virtual_root: VirtualPath::new("/tenants/delete-owner").unwrap(),
                backend_id: BackendId::new("native-delete-owner").unwrap(),
                backend_kind: BackendKind::MemoryDocuments,
                storage_class: StorageClass::FileContent,
                content_kind: ContentKind::ProjectFile,
                index_policy: IndexPolicy::NotIndexed,
                capabilities: backend.capabilities(),
            },
            backend.clone(),
        )
        .unwrap();
    let writable = ScopedFilesystem::with_fixed_view(
        Arc::new(composite),
        view(MountPermissions::read_write_list_delete()),
    );
    assert!(
        writable
            .delete_if_snapshot(&scope, &alias, version, checksum)
            .await
            .unwrap()
    );
    assert!(backend.get(&target).await.unwrap().is_none());
}
#[tokio::test]
async fn local_selected_entry_delete_fails_before_effect_and_overclaim_is_rejected() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("value"), b"local original").unwrap();
    let mut backend = LocalFilesystem::new();
    backend
        .mount_local(
            VirtualPath::new("/resources").unwrap(),
            HostPath::from_path_buf(directory.path().to_path_buf()),
        )
        .unwrap();
    let path = VirtualPath::new("/resources/value").unwrap();
    let selected = backend.get(&path).await.unwrap().unwrap();
    assert!(matches!(
        backend
            .delete_if_snapshot(
                &path,
                selected.version,
                EntryBodyChecksum::for_body(&selected.entry.body)
            )
            .await,
        Err(FilesystemError::Unsupported {
            operation: FilesystemOperation::Delete,
            ..
        })
    ));
    assert_eq!(
        std::fs::read(directory.path().join("value")).unwrap(),
        b"local original"
    );
    let mut composite = CompositeRootFilesystem::new();
    assert!(
        matches!(composite.mount(MountDescriptor { virtual_root: VirtualPath::new("/resources").unwrap(), backend_id: BackendId::new("unsupported-delete-owner").unwrap(), backend_kind: BackendKind::LocalFilesystem, storage_class: StorageClass::FileContent, content_kind: ContentKind::ProjectFile, index_policy: IndexPolicy::NotIndexed, capabilities: backend.capabilities().with(Capability::ConditionalEntryDelete) }, Arc::new(backend)), Err(FilesystemError::DescriptorOverclaims { missing, .. }) if missing.contains(&Capability::ConditionalEntryDelete))
    );
}
