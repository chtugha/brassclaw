//! Real RootFilesystem namespace/CAS contract, reused for native PostgreSQL.
use brassclaw_filesystem::{
    CasExpectation, Entry, FilesystemError, IndexKey, IndexValue, RecordKind, RootFilesystem,
};
use brassclaw_host_api::VirtualPath;
use std::sync::Arc;

pub(crate) async fn assert_namespace_contract(root: Arc<dyn RootFilesystem>) {
    let base = format!("/projects/namespace-{}", uuid::Uuid::new_v4());
    for addressable_root in ["/system/settings", "/system/extensions", "/system/skills"] {
        let parent = VirtualPath::new(format!(
            "{addressable_root}/namespace-{}",
            uuid::Uuid::new_v4()
        ))
        .unwrap();
        root.put(
            &parent,
            Entry::bytes(b"root boundary".to_vec()),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
        assert_eq!(
            root.get(&parent).await.unwrap().unwrap().entry.body,
            b"root boundary"
        );
        let child = VirtualPath::new(format!("{}/child", parent.as_str())).unwrap();
        assert!(
            root.put(
                &child,
                Entry::bytes(b"invalid".to_vec()),
                CasExpectation::Absent
            )
            .await
            .is_err()
        );
    }
    for operation in 0..3 {
        let parent = VirtualPath::new(format!("{base}/existing-{operation}")).unwrap();
        let child = VirtualPath::new(format!("{}/leaf", parent.as_str())).unwrap();
        root.put(
            &parent,
            Entry::bytes(b"original".to_vec()),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
        let result = match operation {
            0 => root
                .put(
                    &child,
                    Entry::bytes(b"bad".to_vec()),
                    CasExpectation::Absent,
                )
                .await
                .map(|_| ()),
            1 => root.write_file(&child, b"bad").await,
            _ => root.append_file(&child, b"bad").await,
        };
        assert!(result.is_err(), "a file cannot become an ancestor");
        assert!(root.get(&child).await.unwrap().is_none());
        assert_eq!(
            root.get(&parent).await.unwrap().unwrap().entry.body,
            b"original"
        );
    }
    // Across the two creation orders, at most one competing file may exist.
    for operation in 0..3 {
        for index in 0..12 {
            let parent = VirtualPath::new(format!("{base}/race-{operation}-{index}")).unwrap();
            let child = VirtualPath::new(format!("{}/leaf", parent.as_str())).unwrap();
            let parent_write = root.put(
                &parent,
                Entry::bytes(b"parent".to_vec()),
                CasExpectation::Absent,
            );
            let child_write = async {
                match operation {
                    0 => root
                        .put(
                            &child,
                            Entry::bytes(b"child".to_vec()),
                            CasExpectation::Absent,
                        )
                        .await
                        .map(|_| ()),
                    1 => root.write_file(&child, b"child").await,
                    _ => root.append_file(&child, b"child").await,
                }
            };
            let (parent_result, child_result) = tokio::join!(parent_write, child_write);
            assert_ne!(
                parent_result.is_ok(),
                child_result.is_ok(),
                "exactly one namespace creation succeeds"
            );
            let parent_entry = root.get(&parent).await.unwrap();
            let child_entry = root.get(&child).await.unwrap();
            assert_ne!(parent_entry.is_some(), child_entry.is_some());
            if let Some(entry) = parent_entry {
                assert_eq!(entry.entry.body, b"parent");
            }
            if let Some(entry) = child_entry {
                assert_eq!(entry.entry.body, b"child");
            }
        }
    }
    let record = VirtualPath::new(format!("{base}/metadata")).unwrap();
    let entry = Entry::record(
        RecordKind::new("fixture").unwrap(),
        &serde_json::json!({"old":1}),
    )
    .unwrap()
    .with_indexed(IndexKey::new("old").unwrap(), IndexValue::I64(1));
    let body = entry.body.clone();
    root.put(&record, entry, CasExpectation::Absent)
        .await
        .unwrap();
    root.append_file(&record, b" appended").await.unwrap();
    let updated = root.get(&record).await.unwrap().unwrap();
    assert!(updated.entry.kind.is_none());
    assert!(updated.entry.indexed.is_empty());
    assert_eq!(
        updated.entry.content_type.as_str(),
        "application/octet-stream"
    );
    assert_eq!(updated.entry.body, [body.as_slice(), b" appended"].concat());
    let expected = updated.version;
    let (left, right) = tokio::join!(
        root.put(
            &record,
            Entry::bytes(b"left".to_vec()),
            CasExpectation::Version(expected)
        ),
        root.put(
            &record,
            Entry::bytes(b"right".to_vec()),
            CasExpectation::Version(expected)
        )
    );
    assert_ne!(left.is_ok(), right.is_ok());
    assert!(
        matches!(left, Err(FilesystemError::VersionMismatch { .. }))
            || matches!(right, Err(FilesystemError::VersionMismatch { .. }))
    );
    assert_eq!(
        root.get(&record).await.unwrap().unwrap().version,
        expected
            .checked_next()
            .expect("test version remains representable")
    );
}
