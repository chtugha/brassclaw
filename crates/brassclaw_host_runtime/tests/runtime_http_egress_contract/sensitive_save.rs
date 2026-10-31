use super::*;
use brassclaw_filesystem::{CasExpectation, Entry, RecordKind};

fn network() -> RecordingNetwork {
    RecordingNetwork::ok(NetworkHttpResponse {
        status: 200,
        headers: vec![],
        body: b"downloaded bytes".to_vec(),
        usage: NetworkUsage {
            request_bytes: 5,
            response_bytes: 16,
            resolved_ip: None,
        },
    })
}

fn request(target: RuntimeHttpSaveTarget) -> RuntimeHttpEgressRequest {
    RuntimeHttpEgressRequest {
        runtime: RuntimeKind::FirstParty,
        scope: sample_scope(),
        capability_id: sample_capability_id(),
        method: NetworkMethod::Get,
        url: "https://api.example.test/v1/run".into(),
        headers: vec![],
        body: b"hello".to_vec(),
        network_policy: sample_policy(),
        credential_injections: vec![],
        response_body_limit: Some(4096),
        save_body_to: Some(target),
        timeout_ms: None,
    }
}

fn mounts(alias: &str, target: &str) -> MountView {
    MountView::new(vec![MountGrant::new(
        MountAlias::new(alias).unwrap(),
        VirtualPath::new(target).unwrap(),
        MountPermissions {
            read: false,
            write: true,
            delete: false,
            list: false,
            execute: false,
        },
    )])
    .unwrap()
}

fn assert_preflight_denied(error: RuntimeHttpEgressError) {
    assert!(matches!(
        error,
        RuntimeHttpEgressError::Request { reason, request_bytes: 0, response_bytes: 0 }
        if reason == "response_body_store_unauthorized"
    ));
}

#[tokio::test]
async fn sensitive_scoped_and_resolved_names_reject_before_network_or_write() {
    for (scoped, resolved) in [
        ("/workspace/.env", "/projects/downloads/.env"),
        ("/workspace/.ssh/config", "/projects/downloads/.ssh/config"),
        ("/workspace/result.txt", "/projects/downloads/.env"),
        ("/workspace/.env", "/projects/downloads/ordinary.txt"),
    ] {
        let root = Arc::new(InMemoryBackend::new());
        let path = VirtualPath::new(resolved).unwrap();
        root.put(
            &path,
            Entry::bytes(b"keep".to_vec()),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
        let original = root.get(&path).await.unwrap();
        let network = network();
        let requests = network.requests.clone();
        let store = Arc::new(ScopedFilesystem::with_fixed_view(
            root.clone(),
            MountView::default(),
        ));
        let service = request_policy_staging_egress_with_body_store(network, store);
        let view = mounts(scoped, resolved);
        let error = service
            .execute(request(save_target_with_mount(scoped, &view)))
            .await
            .unwrap_err();
        assert_preflight_denied(error);
        assert!(requests.lock().unwrap().is_empty());
        assert_eq!(root.get(&path).await.unwrap(), original);
    }
}

#[tokio::test]
async fn hidden_sensitive_local_target_and_parent_reject_before_network() {
    let temp = tempdir().unwrap();
    let secret_dir = temp.path().join(".ssh");
    fs::create_dir(&secret_dir).unwrap();
    fs::write(secret_dir.join("ordinary.txt"), b"keep").unwrap();
    let mut root = LocalFilesystem::new();
    root.mount_local(
        VirtualPath::new("/projects/downloads").unwrap(),
        brassclaw_host_api::HostPath::from_path_buf(secret_dir.clone()),
    )
    .unwrap();
    let store = Arc::new(ScopedFilesystem::with_fixed_view(
        Arc::new(root),
        MountView::default(),
    ));
    for target in ["ordinary.txt", "missing/deep/new.txt"] {
        let network = network();
        let requests = network.requests.clone();
        let service = request_policy_staging_egress_with_body_store(network, store.clone());
        let view = mounts(
            "/workspace/result.txt",
            &format!("/projects/downloads/{target}"),
        );
        let error = service
            .execute(request(save_target_with_mount(
                "/workspace/result.txt",
                &view,
            )))
            .await
            .unwrap_err();
        // The model-visible error must not expose the real mount directory.
        assert!(
            !error
                .to_string()
                .contains(secret_dir.to_string_lossy().as_ref())
        );
        assert_preflight_denied(error);
        assert!(requests.lock().unwrap().is_empty());
        assert_eq!(fs::read(secret_dir.join("ordinary.txt")).unwrap(), b"keep");
        assert!(!secret_dir.join("missing").exists());
    }
}

#[tokio::test]
async fn safe_sample_and_missing_parents_save_with_exact_write_only_grants() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join(".env.example"), b"old sample").unwrap();
    let mut root = LocalFilesystem::new();
    root.mount_local(
        VirtualPath::new("/projects/downloads").unwrap(),
        brassclaw_host_api::HostPath::from_path_buf(temp.path().to_path_buf()),
    )
    .unwrap();
    let store = Arc::new(ScopedFilesystem::with_fixed_view(
        Arc::new(root),
        MountView::default(),
    ));
    for target in [".env.example", "new/deep/file.txt"] {
        let network = network();
        let requests = network.requests.clone();
        let service = request_policy_staging_egress_with_body_store(network, store.clone());
        let scoped = format!("/workspace/{target}");
        let view = mounts(&scoped, &format!("/projects/downloads/{target}"));
        let response = service
            .execute(request(save_target_with_mount(&scoped, &view)))
            .await
            .unwrap();
        assert!(response.body.is_empty());
        let saved = response.saved_body.unwrap();
        assert_eq!(saved.path.as_str(), scoped);
        assert_eq!(saved.bytes_written, 16);
        assert_eq!(requests.lock().unwrap().len(), 1);
        assert_eq!(
            fs::read(temp.path().join(target)).unwrap(),
            b"downloaded bytes"
        );
    }
}

#[tokio::test]
async fn target_becoming_sensitive_during_network_is_preserved_with_usage() {
    let root = Arc::new(InMemoryBackend::new());
    let path = VirtualPath::new("/projects/downloads/state.json").unwrap();
    root.put(
        &path,
        Entry::bytes(b"ordinary".to_vec()),
        CasExpectation::Absent,
    )
    .await
    .unwrap();
    let network = network();
    let requests = network.requests.clone();
    let writing_root = root.clone();
    let writing_path = path.clone();
    let network = BeforeResponseNetwork {
        inner: network,
        before_response: move || {
            block_on_test(
                writing_root.put(
                    &writing_path,
                    Entry::record(
                        RecordKind::new("private_record").unwrap(),
                        &json!({"keep": true}),
                    )
                    .unwrap(),
                    CasExpectation::Any,
                ),
            )
            .unwrap();
        },
    };
    let store = Arc::new(ScopedFilesystem::with_fixed_view(
        root.clone(),
        MountView::default(),
    ));
    let service = request_policy_staging_egress_with_body_store(network, store);
    let view = mounts("/workspace/state.json", path.as_str());
    let error = service
        .execute(request(save_target_with_mount(
            "/workspace/state.json",
            &view,
        )))
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeHttpEgressError::Response { reason, request_bytes: 5, response_bytes: 16 }
        if reason == "response_body_store_failed"
    ));
    assert_eq!(requests.lock().unwrap().len(), 1);
    let entry = root.get(&path).await.unwrap().unwrap().entry;
    assert_eq!(entry.kind, Some(RecordKind::new("private_record").unwrap()));
    assert_eq!(
        serde_json::from_slice::<Value>(&entry.body).unwrap(),
        json!({"keep": true})
    );
}

#[cfg(unix)]
#[tokio::test]
async fn local_alias_becoming_sensitive_during_network_is_not_written() {
    let temp = tempdir().unwrap();
    let output = temp.path().join("output.txt");
    let secret = temp.path().join(".env");
    fs::write(&output, b"ordinary").unwrap();
    fs::write(&secret, b"keep").unwrap();
    let mut root = LocalFilesystem::new();
    root.mount_local(
        VirtualPath::new("/projects/downloads").unwrap(),
        brassclaw_host_api::HostPath::from_path_buf(temp.path().to_path_buf()),
    )
    .unwrap();
    let store = Arc::new(ScopedFilesystem::with_fixed_view(
        Arc::new(root),
        MountView::default(),
    ));
    let network = network();
    let requests = network.requests.clone();
    let alias = output.clone();
    let network = BeforeResponseNetwork {
        inner: network,
        before_response: move || {
            fs::remove_file(&alias).unwrap();
            std::os::unix::fs::symlink(".env", &alias).unwrap();
        },
    };
    let service = request_policy_staging_egress_with_body_store(network, store);
    let view = mounts("/workspace", "/projects/downloads");
    let error = service
        .execute(request(save_target_with_mount(
            "/workspace/output.txt",
            &view,
        )))
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeHttpEgressError::Response { reason, request_bytes: 5, response_bytes: 16 }
        if reason == "response_body_store_failed"
    ));
    assert_eq!(requests.lock().unwrap().len(), 1);
    assert_eq!(fs::read(&secret).unwrap(), b"keep");
    assert_eq!(fs::read(&output).unwrap(), b"keep");
}
