use crate::conditional_backend::{ConditionalBackend, scoped};
use brassclaw_filesystem::{CasExpectation, RootFilesystem};
use brassclaw_host_api::{CapabilityId, InvocationId, ResourceScope, UserId};
use brassclaw_run_state::{FilesystemRunStateStore, RunStart, RunStateStore, RunStatus};
use std::sync::Arc;

pub(crate) async fn verify<F: RootFilesystem + 'static>(inner: Arc<F>) {
    let backend = Arc::new(ConditionalBackend::new(inner));
    let store = FilesystemRunStateStore::new(scoped(backend.clone(), "/run-state"));
    let id = InvocationId::new();
    let scope = ResourceScope::local_default(UserId::new("conditional-run").unwrap(), id).unwrap();
    let request = RunStart {
        invocation_id: id,
        scope: scope.clone(),
        capability_id: CapabilityId::new("echo.say").unwrap(),
    };
    backend.support(false, false);
    assert!(store.start(request.clone()).await.is_err());
    assert!(store.get(&scope, id).await.unwrap().is_none());
    assert_eq!(backend.calls(), vec![CasExpectation::Absent]);

    backend.support(true, false);
    let before = store.start(request).await.unwrap();
    assert!(store.complete(&scope, id).await.is_err());
    assert_eq!(store.get(&scope, id).await.unwrap(), Some(before));
    assert!(
        backend
            .calls()
            .iter()
            .all(|cas| !matches!(cas, CasExpectation::Any))
    );

    backend.support(true, true);
    assert_eq!(
        store.complete(&scope, id).await.unwrap().status,
        RunStatus::Completed
    );
    assert!(
        backend
            .calls()
            .iter()
            .all(|cas| matches!(cas, CasExpectation::Version(_)))
    );
}
