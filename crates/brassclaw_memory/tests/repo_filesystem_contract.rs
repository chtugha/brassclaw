//! Wires [`FilesystemMemoryDocumentRepository`] (over an in-memory
//! [`RootFilesystem`]) to the shared [`MemoryDocumentRepository`]
//! contract suite.
//!
//! See `crates/brassclaw_memory/src/contract_tests.rs` for the suite
//! itself and the rationale (#3890 / .claude/rules/testing.md).
//!
//! Each contract gets its own `InMemoryBackend` — the factory closure
//! constructs both the backing filesystem and the repo per call, so
//! contracts cannot leak state into each other.

use std::sync::Arc;

use brassclaw_filesystem::InMemoryBackend;
use brassclaw_memory::{FilesystemMemoryDocumentRepository, contract_test_indexed};

#[path = "support/archive_contract.rs"]
mod archive_contract;

#[tokio::test]
async fn archive_counters_are_complete_validated_and_never_wrap_or_replay() {
    archive_contract::assert_archive_contract(Arc::new(InMemoryBackend::new())).await;
}

// FilesystemMemoryDocumentRepository implements MemoryDocumentIndexRepository
// and serves FTS search, so it gets the chunk-seeded search-isolation
// contract via `contract_test_indexed!`.
contract_test_indexed!(filesystem, || {
    FilesystemMemoryDocumentRepository::new(Arc::new(InMemoryBackend::new()))
});

#[tokio::test]
async fn full_text_search_preserves_requested_counts_across_filesystem_pages() {
    use brassclaw_memory::{
        MemoryChunkReplaceOutcome, MemoryChunkWrite, MemoryDocumentIndexRepository,
        MemoryDocumentPath, MemoryDocumentRepository, MemoryDocumentScope, MemorySearchRequest,
        content_sha256,
    };
    let repository = FilesystemMemoryDocumentRepository::new(Arc::new(InMemoryBackend::new()));
    let scope = MemoryDocumentScope::new("pagination", "operator", None).unwrap();
    // Cross the former 1,000-result and 5,000-candidate clamps, as well as
    // several actual filesystem pages, using the real index/repository path.
    let count = 5_205;
    for index in 0..count {
        let path = MemoryDocumentPath::new(
            "pagination",
            "operator",
            None,
            format!("notes/{index:05}.md"),
        )
        .unwrap();
        let body = format!("paginationneedle document {index}");
        repository
            .write_document(&path, body.as_bytes())
            .await
            .unwrap();
        let outcome = repository
            .replace_document_chunks_if_current(
                &path,
                &content_sha256(&body),
                &[MemoryChunkWrite {
                    chat_record_id: None,
                    content: body,
                    embedding: None,
                    embedding_space: None,
                }],
            )
            .await
            .unwrap();
        assert_eq!(outcome, MemoryChunkReplaceOutcome::Replaced);
    }
    for requested in [1_050, count] {
        let results = repository
            .search_documents(
                &scope,
                &MemorySearchRequest::new("paginationneedle")
                    .unwrap()
                    .with_limit(requested)
                    .with_pre_fusion_limit(6_000)
                    .with_vector(false),
            )
            .await
            .unwrap();
        assert_eq!(results.len(), requested);
        let paths = results
            .iter()
            .map(|result| result.path.relative_path())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(paths.len(), requested);
        assert!(
            results
                .iter()
                .all(|result| result.snippet.contains("paginationneedle"))
        );
    }
    assert_eq!(
        repository.list_documents(&scope).await.unwrap().len(),
        count
    );
}

#[tokio::test]
async fn narrow_write_preflight_preserves_conflicts_sidecars_and_complete_scope() {
    use brassclaw_memory::{
        MemoryAppendOutcome, MemoryChunkWrite, MemoryDocumentIndexRepository, MemoryDocumentPath,
        MemoryDocumentRepository, MemoryDocumentScope, MemoryWriteOptions, MemoryWriteOutcome,
        content_sha256,
    };
    let repository = FilesystemMemoryDocumentRepository::new(Arc::new(InMemoryBackend::new()));
    let scope =
        MemoryDocumentScope::new_with_agent("tenant", "owner", Some("agent"), Some("project"))
            .unwrap();
    let path = |relative: &str| {
        MemoryDocumentPath::new_with_agent(
            "tenant",
            "owner",
            Some("agent"),
            Some("project"),
            relative,
        )
        .unwrap()
    };
    repository
        .write_document(&path("parent/leaf.md"), b"child")
        .await
        .unwrap();
    repository
        .write_document(&path("file"), b"ancestor")
        .await
        .unwrap();
    for target in [path("parent"), path("file/leaf.md")] {
        assert!(
            repository
                .write_document(&target, b"invalid")
                .await
                .is_err()
        );
        assert!(
            repository
                .compare_and_append_document_with_options(
                    &target,
                    None,
                    b"invalid",
                    &MemoryWriteOptions::default()
                )
                .await
                .is_err()
        );
        assert!(
            repository
                .compare_and_write_document_with_options(
                    &target,
                    None,
                    b"invalid",
                    &MemoryWriteOptions::default()
                )
                .await
                .is_err()
        );
        assert!(repository.read_document(&target).await.unwrap().is_none());
    }
    assert_eq!(
        repository
            .read_document(&path("file"))
            .await
            .unwrap()
            .unwrap(),
        b"ancestor"
    );
    assert_eq!(
        repository
            .read_document(&path("parent/leaf.md"))
            .await
            .unwrap()
            .unwrap(),
        b"child"
    );
    // Unrelated scopes may contain the same names with different relationships.
    for other in [
        MemoryDocumentScope::new_with_agent("other", "owner", Some("agent"), Some("project"))
            .unwrap(),
        MemoryDocumentScope::new_with_agent("tenant", "other", Some("agent"), Some("project"))
            .unwrap(),
        MemoryDocumentScope::new_with_agent("tenant", "owner", None, Some("project")).unwrap(),
        MemoryDocumentScope::new_with_agent("tenant", "owner", Some("agent"), None).unwrap(),
    ] {
        let target = MemoryDocumentPath::new_with_agent(
            other.tenant_id(),
            other.user_id(),
            other.agent_id(),
            other.project_id(),
            "file/leaf.md",
        )
        .unwrap();
        repository
            .write_document(&target, b"isolated")
            .await
            .unwrap();
        assert_eq!(
            repository.list_documents(&other).await.unwrap(),
            vec![target]
        );
    }
    let target = path("notes/versions.md");
    repository.write_document(&target, b"first").await.unwrap();
    repository
        .replace_document_chunks_if_current(
            &target,
            &content_sha256("first"),
            &[MemoryChunkWrite {
                content: "first".into(),
                ..MemoryChunkWrite::default()
            }],
        )
        .await
        .unwrap();
    // Chunk and version siblings must not be mistaken for document descendants.
    repository.write_document(&target, b"second").await.unwrap();
    assert_eq!(
        repository
            .compare_and_append_document_with_options(
                &target,
                Some(&content_sha256("second")),
                b" third",
                &MemoryWriteOptions::default()
            )
            .await
            .unwrap(),
        MemoryAppendOutcome::Appended
    );
    assert_eq!(
        repository
            .compare_and_write_document_with_options(
                &target,
                Some(&content_sha256("second third")),
                b"fourth",
                &MemoryWriteOptions::default()
            )
            .await
            .unwrap(),
        MemoryWriteOutcome::Written
    );
    assert_eq!(
        repository.read_document(&target).await.unwrap().unwrap(),
        b"fourth"
    );
    assert_eq!(repository.list_documents(&scope).await.unwrap().len(), 3);
}

#[tokio::test]
async fn metadata_chunk_cleanup_collects_all_pages_before_deleting() {
    use brassclaw_filesystem::{CasExpectation, Entry, Page, RootFilesystem};
    use brassclaw_host_api::VirtualPath;
    use brassclaw_memory::{
        MemoryChunkReplaceOutcome, MemoryChunkWrite, MemoryDocumentIndexRepository,
        MemoryDocumentPath, MemoryDocumentRepository, content_sha256,
    };
    let filesystem = Arc::new(InMemoryBackend::new());
    let repository = FilesystemMemoryDocumentRepository::new(filesystem.clone());
    let path = MemoryDocumentPath::new("metadata-cleanup", "operator", None, "notes.md").unwrap();
    let count = Page::MAX_LIMIT as usize * 2 + 17;
    let content = "chunk ".repeat(count);
    repository
        .write_document(&path, content.as_bytes())
        .await
        .unwrap();
    let chunks = (0..count)
        .map(|_| MemoryChunkWrite {
            chat_record_id: None,
            content: "chunk".into(),
            embedding: None,
            embedding_space: None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        repository
            .replace_document_chunks_if_current(&path, &content_sha256(&content), &chunks)
            .await
            .unwrap(),
        MemoryChunkReplaceOutcome::Replaced
    );
    let prefix = VirtualPath::new(
        "/memory/tenants/metadata-cleanup/users/operator/agents/_none/projects/_none/notes.md.chunks",
    )
    .unwrap();
    let untouched = VirtualPath::new(format!("{}/untouched", prefix.as_str())).unwrap();
    let body = vec![7; 1024 * 1024];
    filesystem
        .put(
            &untouched,
            Entry::bytes(body.clone()),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
    assert_eq!(
        repository
            .replace_document_chunks_if_current(&path, &content_sha256(&content), &[])
            .await
            .unwrap(),
        MemoryChunkReplaceOutcome::Replaced
    );
    let remaining = filesystem
        .query_metadata(&prefix, Page::default())
        .await
        .unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].path, untouched);
    assert_eq!(
        filesystem
            .get(&untouched)
            .await
            .unwrap()
            .unwrap()
            .entry
            .body,
        body
    );
    assert_eq!(
        repository.read_document(&path).await.unwrap().unwrap(),
        content.as_bytes()
    );
}
