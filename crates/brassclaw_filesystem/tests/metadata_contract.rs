use brassclaw_filesystem::{
    BackendCapabilities, BackendId, BackendKind, Capability, CasExpectation,
    CompositeRootFilesystem, ContentKind, Entry, FilesystemError, InMemoryBackend, IndexPolicy,
    LocalFilesystem, MountDescriptor, Page, RootFilesystem, StorageClass,
};
use brassclaw_host_api::VirtualPath;
use std::sync::Arc;
#[tokio::test]
async fn metadata_routing_and_capability_checks_use_the_existing_mount_boundary() {
    let mut root = CompositeRootFilesystem::new();
    let descriptor = |path: &str, id: &str| MountDescriptor {
        virtual_root: VirtualPath::new(path).unwrap(),
        backend_id: BackendId::new(id).unwrap(),
        backend_kind: BackendKind::Custom("in-memory".into()),
        storage_class: StorageClass::FileContent,
        content_kind: ContentKind::GenericFile,
        index_policy: IndexPolicy::NotIndexed,
        capabilities: BackendCapabilities::empty().with(Capability::EntryMetadata),
    };
    let error = root
        .mount(
            descriptor("/projects", "unsupported"),
            Arc::new(LocalFilesystem::new()),
        )
        .unwrap_err();
    assert!(
        matches!(error,FilesystemError::DescriptorOverclaims { missing,.. } if missing.contains(&Capability::EntryMetadata))
    );
    let broad = Arc::new(InMemoryBackend::new());
    let nested = Arc::new(InMemoryBackend::new());
    let path = VirtualPath::new("/projects/private/item").unwrap();
    broad
        .put(
            &path,
            Entry::bytes(b"broad".to_vec()),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
    nested
        .put(
            &path,
            Entry::record(
                brassclaw_filesystem::RecordKind::new("nested").unwrap(),
                &serde_json::Value::Null,
            )
            .unwrap(),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
    root.mount(descriptor("/projects", "broad"), broad).unwrap();
    root.mount(descriptor("/projects/private", "private"), nested)
        .unwrap();
    let exact = root.get_metadata(&path).await.unwrap().unwrap();
    assert_eq!(exact.kind.unwrap().as_str(), "nested");
    let page = root
        .query_metadata(
            &VirtualPath::new("/projects/private").unwrap(),
            Page::default(),
        )
        .await
        .unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].kind.as_ref().unwrap().as_str(), "nested");
    let indexed = root
        .query_index_metadata(
            &path,
            &brassclaw_filesystem::IndexKey::new("missing").unwrap(),
            Page::default(),
        )
        .await
        .unwrap();
    assert_eq!(indexed.len(), 1);
    assert_eq!(
        indexed[0].metadata.kind.as_ref().unwrap().as_str(),
        "nested"
    );
    assert!(indexed[0].value.is_none());
    let byte_only = LocalFilesystem::new();
    assert!(!byte_only.capabilities().has(Capability::EntryMetadata));
    assert!(matches!(
        byte_only.query_metadata(&path, Page::default()).await,
        Err(FilesystemError::Unsupported { .. })
    ));
}
