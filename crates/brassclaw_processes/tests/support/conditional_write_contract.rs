use crate::conditional_backend::{ConditionalBackend, scoped};
use brassclaw_filesystem::{CasExpectation, RootFilesystem};
use brassclaw_host_api::{
    CapabilityId, CapabilitySet, ExtensionId, InvocationId, MountView, ProcessId, ResourceEstimate,
    ResourceScope, RuntimeKind, UserId,
};
use brassclaw_processes::{
    FilesystemProcessStore, ProcessError, ProcessStart, ProcessStatus, ProcessStore,
};
use std::sync::Arc;

pub(crate) async fn verify<F: RootFilesystem + 'static>(inner: Arc<F>) {
    let backend = Arc::new(ConditionalBackend::new(inner));
    let filesystem = scoped(backend.clone(), "/processes");
    let store = FilesystemProcessStore::new(filesystem.clone());
    let id = ProcessId::new();
    let invocation_id = InvocationId::new();
    let scope =
        ResourceScope::local_default(UserId::new("conditional-process").unwrap(), invocation_id)
            .unwrap();
    let request = ProcessStart {
        process_id: id,
        parent_process_id: None,
        invocation_id,
        scope: scope.clone(),
        extension_id: ExtensionId::new("echo").unwrap(),
        capability_id: CapabilityId::new("echo.say").unwrap(),
        runtime: RuntimeKind::Mcp,
        grants: CapabilitySet { grants: Vec::new() },
        mounts: MountView::default(),
        estimated_resources: ResourceEstimate::default(),
        resource_reservation_id: None,
        input: serde_json::json!({}),
    };
    backend.support(false, false);
    assert!(store.start(request.clone()).await.is_err());
    assert!(store.get(&scope, id).await.unwrap().is_none());
    assert_eq!(
        backend.calls(),
        vec![CasExpectation::Absent, CasExpectation::Absent]
    );

    backend.support(true, false);
    let before = store.start(request.clone()).await.unwrap();
    assert!(store.kill(&scope, id).await.is_err());
    assert_eq!(store.get(&scope, id).await.unwrap(), Some(before));
    assert!(
        backend
            .calls()
            .iter()
            .all(|cas| !matches!(cas, CasExpectation::Any))
    );
    backend.support(true, true);
    assert_eq!(
        store.kill(&scope, id).await.unwrap().status,
        ProcessStatus::Killed
    );
    assert_eq!(backend.calls().len(), 2);
    assert!(
        backend
            .calls()
            .iter()
            .all(|cas| matches!(cas, CasExpectation::Version(_)))
    );

    // Both independent callers reach the final publication barrier before
    // either writes. Only the real owner's atomic Absent may win.
    let first = FilesystemProcessStore::new(filesystem.clone());
    let second = FilesystemProcessStore::new(filesystem);
    let request = ProcessStart {
        process_id: ProcessId::new(),
        ..request
    };
    let mut competing = request.clone();
    competing.capability_id = CapabilityId::new("echo.other").unwrap();
    *backend.absent_barrier.lock().unwrap() = Some(Arc::new(tokio::sync::Barrier::new(2)));
    let (a, b) = tokio::join!(first.start(request.clone()), second.start(competing));
    let winner = match (a, b) {
        (Ok(winner), Err(ProcessError::ProcessAlreadyExists { .. }))
        | (Err(ProcessError::ProcessAlreadyExists { .. }), Ok(winner)) => winner,
        other => panic!("one atomic original required: {other:?}"),
    };
    assert_eq!(
        first.get(&scope, request.process_id).await.unwrap(),
        Some(winner)
    );
}
