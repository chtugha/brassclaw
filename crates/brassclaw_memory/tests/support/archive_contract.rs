//! Exercise the real filesystem-backed repository's archive bookkeeping.
use std::sync::Arc;

use brassclaw_filesystem::{
    CasExpectation, Entry, FilesystemError, IndexKey, IndexValue, Page, RootFilesystem,
};
use brassclaw_host_api::VirtualPath;
use brassclaw_memory::{
    FilesystemMemoryDocumentRepository, MemoryDocumentPath, MemoryDocumentRepository,
    MemoryWriteOptions, MemoryWriteOutcome, content_sha256,
};

pub(crate) async fn assert_archive_contract(filesystem: Arc<dyn RootFilesystem>) {
    let repository = FilesystemMemoryDocumentRepository::new(filesystem.clone());
    let counter = IndexKey::new("version").unwrap();
    let archive_path = |name: &str, version: &str| {
        VirtualPath::new(format!(
            "/memory/tenants/archive-contract/users/operator/agents/_none/projects/_none/{name}.md.versions/{version}"
        )).unwrap()
    };
    let document = |name: &str| {
        MemoryDocumentPath::new("archive-contract", "operator", None, format!("{name}.md")).unwrap()
    };

    let path = document("many");
    repository.write_document(&path, b"first").await.unwrap();
    repository.write_document(&path, b"second").await.unwrap();
    let template = filesystem
        .get(&archive_path("many", "1"))
        .await
        .unwrap()
        .unwrap()
        .entry;
    let total = Page::MAX_LIMIT as i64 + 17;
    for version in 2..=total {
        filesystem
            .put(
                &archive_path("many", &version.to_string()),
                template
                    .clone()
                    .with_indexed(counter.clone(), IndexValue::I64(version)),
                CasExpectation::Absent,
            )
            .await
            .unwrap();
    }
    // An unrelated record is not an archive, even with a large valid counter.
    filesystem
        .put(
            &archive_path("many", "unrelated"),
            Entry::bytes(vec![]).with_indexed(counter.clone(), IndexValue::I64(i64::MAX)),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
    repository.write_document(&path, b"third").await.unwrap();
    let latest = filesystem
        .get(&archive_path("many", &(total + 1).to_string()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        latest.entry.indexed.get(&counter),
        Some(&IndexValue::I64(total + 1))
    );
    let payload: serde_json::Value = serde_json::from_slice(&latest.entry.body).unwrap();
    assert_eq!(payload["content"], "second");

    for (name, address, value) in [
        ("missing", "2", None),
        ("text", "2", Some(IndexValue::Text("2".into()))),
        ("zero", "0", Some(IndexValue::I64(0))),
        ("negative", "-1", Some(IndexValue::I64(-1))),
        ("mismatch", "2", Some(IndexValue::I64(3))),
        ("noncanonical", "02", Some(IndexValue::I64(2))),
    ] {
        let path = document(name);
        repository.write_document(&path, b"first").await.unwrap();
        repository.write_document(&path, b"second").await.unwrap();
        let mut invalid = template.clone();
        invalid.indexed.remove(&counter);
        if let Some(value) = value {
            invalid.indexed.insert(counter.clone(), value);
        }
        filesystem
            .put(
                &archive_path(name, address),
                invalid,
                CasExpectation::Absent,
            )
            .await
            .unwrap();
        let outcome = repository
            .compare_and_write_document_with_options(
                &path,
                Some(&content_sha256("second")),
                b"committed",
                &MemoryWriteOptions::default(),
            )
            .await;
        assert!(
            matches!(outcome, Err(FilesystemError::Backend { .. })),
            "invalid archive index must be an error, never a no-effect conflict: {outcome:?}"
        );
        assert_eq!(
            repository.read_document(&path).await.unwrap().unwrap(),
            b"committed"
        );
        assert_eq!(
            repository
                .compare_and_write_document_with_options(
                    &path,
                    Some(&content_sha256("second")),
                    b"replayed",
                    &MemoryWriteOptions::default()
                )
                .await
                .unwrap(),
            MemoryWriteOutcome::Conflict
        );
        assert_eq!(
            repository.read_document(&path).await.unwrap().unwrap(),
            b"committed"
        );
    }

    let path = document("boundary");
    repository.write_document(&path, b"first").await.unwrap();
    repository.write_document(&path, b"second").await.unwrap();
    filesystem
        .put(
            &archive_path("boundary", &(i64::MAX - 1).to_string()),
            template.with_indexed(counter.clone(), IndexValue::I64(i64::MAX - 1)),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
    repository.write_document(&path, b"third").await.unwrap();
    let last = filesystem
        .get(&archive_path("boundary", &i64::MAX.to_string()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        last.entry.indexed.get(&counter),
        Some(&IndexValue::I64(i64::MAX))
    );
    let outcome = repository
        .compare_and_write_document_with_options(
            &path,
            Some(&content_sha256("third")),
            b"final",
            &MemoryWriteOptions::default(),
        )
        .await;
    assert!(
        matches!(outcome, Err(FilesystemError::Backend { .. })),
        "exhaustion must not wrap or become a retryable conflict: {outcome:?}"
    );
    assert_eq!(
        repository.read_document(&path).await.unwrap().unwrap(),
        b"final"
    );
    assert!(
        filesystem
            .get(&archive_path("boundary", "-9223372036854775808"))
            .await
            .unwrap()
            .is_none()
    );
}
