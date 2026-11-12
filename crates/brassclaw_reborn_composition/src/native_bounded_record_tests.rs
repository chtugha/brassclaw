//! Actual PostgreSQL projection and longest-mount behavior; no substitute backend.
use brassclaw_filesystem::{BoundedRecordBodyRead as Read, *};
use brassclaw_host_api::VirtualPath;
use secrecy::ExposeSecret;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pg_bounded_record_projection_preserves_original_versions_and_rejects_oversize() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let pool = brassclaw_pg::pool::build_pool_with_max_size(rig.url.expose_secret(), 1).unwrap();
    let backend = Arc::new(PostgresRootFilesystem::new(pool.clone()));
    backend.run_migrations().await.unwrap();
    let other = PostgresRootFilesystem::new(pool.clone());
    let kind = RecordKind::new("bounded_test").unwrap();
    let path = VirtualPath::new("/memory/bounded/value").unwrap();
    let mut entry = Entry::record(kind.clone(), &serde_json::Value::Null).unwrap();
    entry.body = b"actual original bytes".to_vec();
    // The requested projection must not decode even malformed, unrequested
    // indexed metadata. This real DB fixture is restored by the real put below.
    let first = backend
        .put(&path, entry.clone(), CasExpectation::Absent)
        .await
        .unwrap();
    let client = pool.get().await.unwrap();
    client.execute("UPDATE root_filesystem_entries SET indexed=jsonb_build_object('unrequested',repeat('x',octet_length(contents)+1)) WHERE path=$1",&[&path.as_str()]).await.unwrap();
    drop(client);
    let descriptor = |prefix: &str, id: &str, caps| MountDescriptor {
        virtual_root: VirtualPath::new(prefix).unwrap(),
        backend_id: BackendId::new(id).unwrap(),
        backend_kind: BackendKind::DatabaseFilesystem,
        storage_class: StorageClass::StructuredRecords,
        content_kind: ContentKind::MemoryDocument,
        index_policy: IndexPolicy::NotIndexed,
        capabilities: caps,
    };
    let mut root = CompositeRootFilesystem::new();
    root.mount(
        descriptor("/memory", "actual-pg", backend.capabilities()),
        backend.clone(),
    )
    .unwrap();
    let Read::Present(snapshot) = root
        .get_record_body_bounded(&path, &kind, entry.body.len())
        .await
        .unwrap()
    else {
        panic!("exact limit present");
    };
    assert_eq!(snapshot.path, path);
    assert_eq!(snapshot.kind, kind);
    assert_eq!(snapshot.version, first);
    assert_eq!(snapshot.body, entry.body);
    assert_eq!(
        root.get_record_body_bounded(&path, &kind, entry.body.len() - 1)
            .await
            .unwrap(),
        Read::TooLarge {
            body_len: entry.body.len() as u64
        }
    );
    assert_eq!(
        root.get_record_body_bounded(
            &VirtualPath::new("/memory/bounded/missing").unwrap(),
            &kind,
            0
        )
        .await
        .unwrap(),
        Read::Missing
    );
    assert!(
        root.get_record_body_bounded(&path, &RecordKind::new("different").unwrap(), 0)
            .await
            .is_err()
    );
    let empty = VirtualPath::new("/memory/bounded/empty").unwrap();
    let mut empty_entry = entry.clone();
    empty_entry.body.clear();
    let empty_version = backend
        .put(&empty, empty_entry, CasExpectation::Absent)
        .await
        .unwrap();
    let Read::Present(empty_snapshot) = root
        .get_record_body_bounded(&empty, &kind, 0)
        .await
        .unwrap()
    else {
        panic!("empty is not NULL/missing");
    };
    assert!(empty_snapshot.body.is_empty());
    assert_eq!(empty_snapshot.version, empty_version);
    let opaque = VirtualPath::new("/memory/bounded/opaque").unwrap();
    backend
        .put(&opaque, Entry::bytes(Vec::new()), CasExpectation::Absent)
        .await
        .unwrap();
    assert!(
        root.get_record_body_bounded(&opaque, &kind, 0)
            .await
            .is_err()
    );
    let directory = VirtualPath::new("/memory/bounded/directory").unwrap();
    backend.create_dir_all(&directory).await.unwrap();
    assert!(
        root.get_record_body_bounded(&directory, &kind, 0)
            .await
            .is_err()
    );

    // Real RR preflight, then an independent committed writer, with no reader
    // connection held. The new statement sees the new whole row atomically.
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    assert_eq!(
        tx.query_one(
            "SELECT version FROM root_filesystem_entries WHERE path=$1",
            &[&path.as_str()]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        first.get() as i64
    );
    tx.commit().await.unwrap();
    drop(client);
    let second = other
        .put(&path, entry.clone(), CasExpectation::Version(first))
        .await
        .unwrap();
    let Read::Present(same_bytes) = root
        .get_record_body_bounded(&path, &kind, entry.body.len())
        .await
        .unwrap()
    else {
        panic!("same bytes, actual new version");
    };
    assert_eq!(same_bytes.body, entry.body);
    assert_eq!(same_bytes.version, second);
    assert_ne!(second, first);
    let limit = entry.body.len();
    entry.body.push(b'!');
    let third = other
        .put(&path, entry.clone(), CasExpectation::Version(second))
        .await
        .unwrap();
    assert_eq!(
        root.get_record_body_bounded(&path, &kind, limit)
            .await
            .unwrap(),
        Read::TooLarge {
            body_len: entry.body.len() as u64
        }
    );
    let client = pool.get().await.unwrap();
    client
        .execute(
            "UPDATE root_filesystem_entries SET version=-1 WHERE path=$1",
            &[&path.as_str()],
        )
        .await
        .unwrap();
    drop(client);
    assert!(matches!(
        root.get_record_body_bounded(&path, &kind, entry.body.len())
            .await,
        Err(FilesystemError::CorruptRecordVersion { .. })
    ));
    let client = pool.get().await.unwrap();
    client
        .execute(
            "UPDATE root_filesystem_entries SET version=$2 WHERE path=$1",
            &[&path.as_str(), &(third.get() as i64)],
        )
        .await
        .unwrap();
    drop(client);

    let local = Arc::new(LocalFilesystem::new());
    root.mount(
        descriptor(
            "/memory/bounded/private",
            "unsupported-nested",
            local.capabilities(),
        ),
        local.clone(),
    )
    .unwrap();
    let nested = VirtualPath::new("/memory/bounded/private/value").unwrap();
    backend
        .put(&nested, entry.clone(), CasExpectation::Absent)
        .await
        .unwrap();
    assert!(
        matches!(
            root.get_record_body_bounded(&nested, &kind, entry.body.len())
                .await,
            Err(FilesystemError::Unsupported { .. })
        ),
        "never fall back to broader PG mount"
    );
    let sibling = VirtualPath::new("/memory/bounded/private-suffix/value").unwrap();
    backend
        .put(&sibling, entry.clone(), CasExpectation::Absent)
        .await
        .unwrap();
    assert!(
        matches!(
            root.get_record_body_bounded(&sibling, &kind, entry.body.len())
                .await
                .unwrap(),
            Read::Present(_)
        ),
        "component boundary, not string prefix"
    );
    assert!(
        matches!(root.mount(descriptor("/tmp/overclaim","overclaim",BackendCapabilities::empty().with(Capability::BoundedRecordBody)),local),Err(FilesystemError::DescriptorOverclaims {missing,..}) if missing.contains(&Capability::BoundedRecordBody))
    );
    assert!(matches!(
        HsmBackend::new()
            .get_record_body_bounded(&path, &kind, limit)
            .await,
        Err(FilesystemError::Unsupported { .. })
    ));
    assert!(matches!(
        root.get_record_body_bounded(&VirtualPath::new("/tmp/no-mount").unwrap(), &kind, limit)
            .await,
        Err(FilesystemError::MountNotFound { .. })
    ));
}
