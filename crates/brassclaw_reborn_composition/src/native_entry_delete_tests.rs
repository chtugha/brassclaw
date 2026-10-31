//! Actual native transactions and protocol-level lost acknowledgement.
use brassclaw_filesystem::{
    CasExpectation, Entry, EntryBodyChecksum, PostgresRootFilesystem, RootFilesystem,
};
use brassclaw_host_api::VirtualPath;
use std::sync::Arc;
#[path = "../../brassclaw_filesystem/tests/support/conditional_entry_delete_contract.rs"]
mod contract;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pg_selected_entry_delete_fences_actual_independent_owners() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let first = PostgresRootFilesystem::new(rig.pool.as_ref().clone());
    first.run_migrations().await.unwrap();
    let second = PostgresRootFilesystem::new(rig.pool.as_ref().clone());
    // Arrange a real explicit native directory, distinct from inferred parents.
    let directory =
        VirtualPath::new("/tenants/conditional-entry-delete/explicit-directory").unwrap();
    first.create_dir_all(&directory).await.unwrap();
    assert!(
        !first
            .delete_if_snapshot(
                &directory,
                brassclaw_filesystem::RecordVersion::from_backend(0),
                EntryBodyChecksum::for_body(b"")
            )
            .await
            .unwrap()
    );
    assert_eq!(
        first.stat(&directory).await.unwrap().file_type,
        brassclaw_filesystem::FileType::Directory
    );
    contract::verify(&first, &second).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pg_selected_entry_delete_retains_unknown_commit_without_deleting_a_successor() {
    use brassclaw_pg::test_support::{AckCutRule, AckCutTransport};
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let reader = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    reader.run_migrations().await.unwrap();
    let path = VirtualPath::new("/tenants/conditional-delete-commit/value").unwrap();
    let version = reader
        .put(
            &path,
            Entry::bytes(b"delete original".to_vec()),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
    let checksum = EntryBodyChecksum::for_body(b"delete original");
    let transport =
        AckCutTransport::start(rig.url.expose_secret(), vec![AckCutRule::next_commit(1)])
            .await
            .unwrap();
    let proxied = PostgresRootFilesystem::new(transport.pool.as_ref().clone());
    transport.arm(1).unwrap();
    assert!(
        proxied
            .delete_if_snapshot(&path, version, checksum)
            .await
            .is_err()
    );
    assert_eq!(transport.cut_count(), 1);
    assert!(reader.get(&path).await.unwrap().is_none());
    assert_eq!(
        reader
            .put(
                &path,
                Entry::bytes(b"new successor".to_vec()),
                CasExpectation::Absent
            )
            .await
            .unwrap(),
        version
    );
    assert!(
        !reader
            .delete_if_snapshot(&path, version, checksum)
            .await
            .unwrap()
    );
    assert_eq!(
        reader.get(&path).await.unwrap().unwrap().entry.body,
        b"new successor"
    );
    drop(proxied);
    transport.shutdown().await.unwrap();
}
