//! Same real repository/VFS contract over InMemory and PostgreSQL RootFilesystem.
use std::{collections::HashSet, sync::Arc};

use brassclaw_filesystem::{IndexKey, IndexValue, Page, RootFilesystem};
use brassclaw_host_api::{VirtualPath, ids::MAX_SCOPE_ID_BYTES};
use brassclaw_memory::{
    FilesystemMemoryDocumentRepository, MemoryBackend, MemoryBackendFilesystemAdapter,
    MemoryChunkReplaceOutcome, MemoryChunkWrite, MemoryContext, MemoryDocumentFilesystem,
    MemoryDocumentIndexRepository, MemoryDocumentPath, MemoryDocumentRepository,
    MemoryDocumentScope, RepositoryMemoryBackend, content_sha256, fs_keys,
};
use serde_json::{Value, json};

pub(crate) async fn assert_round_trips(root: Arc<dyn RootFilesystem>) {
    let repository = Arc::new(FilesystemMemoryDocumentRepository::new(root.clone()));
    let files = MemoryDocumentFilesystem::new(repository.clone());
    let backend = Arc::new(RepositoryMemoryBackend::new(repository.clone()));
    let backend_files = MemoryBackendFilesystemAdapter::new(backend.clone());
    let mut scopes = Vec::new();
    // All independent optional-axis combinations, not a correlated pair.
    for agent in [None, Some("_none"), Some("opaque:agent-Ü")] {
        for project in [None, Some("_none"), Some("opaque:project-Ü")] {
            scopes.push(
                MemoryDocumentScope::new_with_agent("scope-contract", "operator", agent, project)
                    .unwrap(),
            );
        }
    }
    for (tenant, user, agent, project) in [
        (" ", "operator", None, None),
        ("scope-contract", "\u{2003}", None, None),
        ("scope-contract", "operator", Some(" "), None),
        ("scope-contract", "operator", None, Some("\u{2003}")),
        (
            "tenant:colon",
            "user:colon",
            Some("agent:colon"),
            Some("project:colon"),
        ),
        ("scopes-v2", "s5f6e6f6e65%3A", Some("Ü"), Some("n")),
        ("_none", "_none", None, None),
        // Delimiter-shaped values must never become the same attribution tuple.
        ("a:user:b", "c", None, None),
        ("a", "b:user:c", None, None),
    ] {
        scopes.push(MemoryDocumentScope::new_with_agent(tenant, user, agent, project).unwrap());
    }
    let maximum = format!(":{}", "ü".repeat((MAX_SCOPE_ID_BYTES - 2) / 2)) + "x";
    assert_eq!(maximum.len(), MAX_SCOPE_ID_BYTES);
    scopes.push(
        MemoryDocumentScope::new_with_agent(
            maximum.clone(),
            maximum.clone(),
            Some(&maximum),
            Some(&maximum),
        )
        .unwrap(),
    );
    let mut addresses = HashSet::new();
    let mut attributions = HashSet::new();
    for (index, scope) in scopes.iter().enumerate() {
        // A short real document path also exercises PostgreSQL's actual index
        // key constraints with four maximum-byte IDs; no hypothetical FS limit.
        let path = MemoryDocumentPath::new_with_agent(
            scope.tenant_id(),
            scope.user_id(),
            scope.agent_id(),
            scope.project_id(),
            "n.md",
        )
        .unwrap();
        let before: HashSet<_> = root
            .query_metadata(
                &VirtualPath::new("/memory").unwrap(),
                Page::first(Page::MAX_LIMIT),
            )
            .await
            .unwrap()
            .into_iter()
            .map(|entry| entry.path)
            .collect();
        let body = format!("original-{index}");
        repository
            .write_document(&path, body.as_bytes())
            .await
            .unwrap();
        let added: Vec<_> = root
            .query_metadata(
                &VirtualPath::new("/memory").unwrap(),
                Page::first(Page::MAX_LIMIT),
            )
            .await
            .unwrap()
            .into_iter()
            .filter(|entry| {
                !before.contains(&entry.path)
                    && entry
                        .kind
                        .as_ref()
                        .is_some_and(|kind| kind.as_str() == "memory_document")
            })
            .collect();
        assert_eq!(added.len(), 1);
        let address = &added[0].path;
        assert!(
            addresses.insert(address.clone()),
            "different scopes must have different keys"
        );
        if index == 0 {
            assert_eq!(
                address.as_str(),
                "/memory/tenants/scope-contract/users/operator/agents/_none/projects/_none/n.md"
            );
        } else if scope.tenant_id() == "scopes-v2" {
            assert_eq!(
                address.as_str(),
                "/memory/tenants/scopes-v2/users/s5f6e6f6e65%3A/agents/Ü/projects/n/n.md"
            );
        } else if scope.tenant_id() == "_none" {
            assert_eq!(
                address.as_str(),
                "/memory/tenants/_none/users/_none/agents/_none/projects/_none/n.md"
            );
        } else {
            assert!(address.as_str().starts_with("/memory/scopes-v2/"));
        }
        if index == 1 {
            assert_eq!(
                address.as_str(),
                "/memory/scopes-v2/t/s73636f70652d636f6e7472616374/u/s6f70657261746f72/a/n/p/s5f6e6f6e65/n.md"
            );
        }
        let entry = root.get(address).await.unwrap().unwrap().entry;
        for (key, value) in [
            (fs_keys::TENANT, Some(scope.tenant_id())),
            (fs_keys::USER, Some(scope.user_id())),
            (fs_keys::AGENT, scope.agent_id()),
            (fs_keys::PROJECT, scope.project_id()),
        ] {
            assert_eq!(
                entry.indexed.get(&IndexKey::new(key).unwrap()),
                value.map(|v| IndexValue::Text(v.into())).as_ref()
            );
        }
        assert_eq!(files.read_file(address).await.unwrap(), body.as_bytes());
        assert_eq!(
            backend_files.read_file(address).await.unwrap(),
            body.as_bytes()
        );
        assert_eq!(
            files
                .list_dir(&VirtualPath::new(address.as_str().trim_end_matches("/n.md")).unwrap())
                .await
                .unwrap()
                .len(),
            1
        );
        files.write_file(address, b"second").await.unwrap();
        backend_files.write_file(address, b"third").await.unwrap();
        let reopened = FilesystemMemoryDocumentRepository::new(root.clone());
        assert_eq!(
            reopened.read_document(&path).await.unwrap().unwrap(),
            b"third"
        );
        assert_eq!(
            reopened.list_documents(scope).await.unwrap(),
            vec![path.clone()]
        );
        let version = root
            .get(&VirtualPath::new(format!("{}.versions/1", address.as_str())).unwrap())
            .await
            .unwrap()
            .unwrap();
        let version: Value = serde_json::from_slice(&version.entry.body).unwrap();
        assert_eq!(version["content"], body);
        let attribution = version["changed_by"].as_str().unwrap();
        assert!(attributions.insert(attribution.to_owned()));
        if index == 0 {
            assert_eq!(
                attribution,
                "tenant:scope-contract:user:operator:project:_none"
            );
        } else if index == 1 {
            assert_eq!(
                attribution,
                "memory-owner/2:[\"scope-contract\",\"operator\",\"_none\"]"
            );
        } else if index == 3 {
            assert_eq!(
                attribution,
                "memory-changed-by/2:[\"scope-contract\",\"operator\",\"_none\",null]"
            );
        }
        let second_version = root
            .get(&VirtualPath::new(format!("{}.versions/2", address.as_str())).unwrap())
            .await
            .unwrap()
            .unwrap();
        let second_version: Value = serde_json::from_slice(&second_version.entry.body).unwrap();
        assert_eq!(second_version["content"], "second");
        assert_eq!(second_version["changed_by"], version["changed_by"]);
        if let Some(tuple) = attribution.strip_prefix("memory-changed-by/2:") {
            assert_eq!(
                serde_json::from_str::<Value>(tuple).unwrap(),
                json!([
                    scope.tenant_id(),
                    scope.user_id(),
                    scope.agent_id().unwrap(),
                    scope.project_id()
                ])
            );
        } else if let Some(tuple) = attribution.strip_prefix("memory-owner/2:") {
            assert_eq!(
                serde_json::from_str::<Value>(tuple).unwrap(),
                json!([scope.tenant_id(), scope.user_id(), scope.project_id()])
            );
        } else {
            let expected = match scope.agent_id() {
                None => format!(
                    "tenant:{}:user:{}:project:{}",
                    scope.tenant_id(),
                    scope.user_id(),
                    scope.project_id().unwrap_or("_none")
                ),
                Some(agent) => format!(
                    "tenant:{}:user:{}:agent:{agent}:project:{}",
                    scope.tenant_id(),
                    scope.user_id(),
                    scope.project_id().unwrap_or("_none")
                ),
            };
            assert_eq!(attribution, expected);
        }
        let metadata = json!({"original_scope_tag": index});
        reopened
            .write_document_metadata(&path, &metadata)
            .await
            .unwrap();
        assert_eq!(
            reopened.read_document_metadata(&path).await.unwrap(),
            Some(metadata)
        );
        assert!(
            root.get(&VirtualPath::new(format!("{}.meta", address.as_str())).unwrap())
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(
            reopened
                .replace_document_chunks_if_current(
                    &path,
                    &content_sha256("third"),
                    &[MemoryChunkWrite {
                        chat_record_id: None,
                        content: "third".into(),
                        embedding: None,
                        embedding_space: None,
                    }]
                )
                .await
                .unwrap(),
            MemoryChunkReplaceOutcome::Replaced
        );
        assert!(
            root.get(&VirtualPath::new(format!("{}.chunks/0", address.as_str())).unwrap())
                .await
                .unwrap()
                .is_some()
        );
        let outside =
            MemoryContext::new(MemoryDocumentScope::new("outside-scope", "other", None).unwrap());
        assert!(
            backend
                .write_document(&outside, &path, b"denied")
                .await
                .is_err()
        );
        assert_eq!(
            reopened.read_document(&path).await.unwrap().unwrap(),
            b"third"
        );
    }
    // Preserve the documented input-only no-agent compatibility alias.
    let alias =
        VirtualPath::new("/memory/tenants/scope-contract/users/operator/projects/_none/legacy.md")
            .unwrap();
    files.write_file(&alias, b"legacy").await.unwrap();
    assert_eq!(root.get(&VirtualPath::new("/memory/tenants/scope-contract/users/operator/agents/_none/projects/_none/legacy.md").unwrap()).await.unwrap().unwrap().entry.body, b"legacy");
    assert!(root.get(&alias).await.unwrap().is_none());
}

pub(crate) async fn assert_invalid_addresses(root: Arc<dyn RootFilesystem>) {
    let repository = Arc::new(FilesystemMemoryDocumentRepository::new(root.clone()));
    let files = MemoryDocumentFilesystem::new(repository.clone());
    let backend_files =
        MemoryBackendFilesystemAdapter::new(Arc::new(RepositoryMemoryBackend::new(repository)));
    let prefix = "/memory/scopes-v2/t/s74/u/s75/a/s5f6e6f6e65/p/n";
    let mut bad = vec![
        "/memory/scopes-v2/t/s74/u/s75/a/n/p/n/n.md".to_owned(), // legacy scope in v2
        "/memory/tenants/t/users/u/agents/a:colon/projects/_none/n.md".into(),
        "/memory/tenants/ /users/u/agents/_none/projects/_none/n.md".into(),
    ];
    for token in [
        "s",
        "s0",
        "sGG",
        "s5F6e6f6e65",
        "sff",
        "s00",
        "s2f",
        "s5c",
        "s2e",
        "s2e2e",
        "n",
        "%5fnone",
    ] {
        bad.push(format!(
            "/memory/scopes-v2/t/{token}/u/s75/a/s5f6e6f6e65/p/n/n.md"
        ));
    }
    bad.push(format!(
        "/memory/scopes-v2/t/s{}/u/s75/a/s5f6e6f6e65/p/n/n.md",
        "61".repeat(MAX_SCOPE_ID_BYTES + 1)
    ));
    for suffix in [
        "",
        "/x.meta",
        "/x.chunks/y",
        "/x.versions/1",
        "/../escape",
        "/x\\y",
    ] {
        bad.push(format!("{prefix}{suffix}"));
    }
    bad.push(prefix.replace("/a/", "/agent/") + "/n.md");
    bad.push(prefix.replace("/p/n", "/n") + "/n.md");
    bad.push(prefix.replace("/scopes-v2/", "/SCOPES-V2/") + "/n.md");
    for token in ["N", "n0", "none", "_none", "s", "s5F6e6f6e65"] {
        bad.push(prefix.replace("/a/s5f6e6f6e65/", &format!("/a/{token}/")) + "/n.md");
        bad.push(prefix.replace("/p/n", &format!("/p/{token}")) + "/n.md");
    }
    for raw in bad {
        if let Ok(path) = VirtualPath::new(raw) {
            assert!(files.write_file(&path, b"denied").await.is_err(), "{path}");
            assert!(
                backend_files.write_file(&path, b"denied").await.is_err(),
                "{path}"
            );
        }
    }
    assert!(
        root.query_metadata(
            &VirtualPath::new("/memory").unwrap(),
            Page::first(Page::MAX_LIMIT)
        )
        .await
        .unwrap()
        .is_empty()
    );
}
