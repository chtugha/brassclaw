//! Real-owner delegation with deliberately unavailable projections/CAS modes.
//! Rejection happens before forwarding a write; accepted operations use the
//! supplied actual owner unchanged. This never manufactures successful writes.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;
use brassclaw_filesystem::{
    CasExpectation, DirEntry, Entry, FileStat, FilesystemError, FilesystemOperation, RecordVersion,
    RootFilesystem, ScopedFilesystem, VersionedEntry,
};
use brassclaw_host_api::{MountAlias, MountGrant, MountPermissions, MountView, VirtualPath};

pub(crate) struct ConditionalBackend<F> {
    pub(crate) inner: Arc<F>,
    allow_absent: AtomicBool,
    allow_version: AtomicBool,
    calls: Mutex<Vec<CasExpectation>>,
    pub(crate) absent_barrier: Mutex<Option<Arc<tokio::sync::Barrier>>>,
}

impl<F: RootFilesystem> ConditionalBackend<F> {
    pub(crate) fn new(inner: Arc<F>) -> Self {
        Self {
            inner,
            allow_absent: AtomicBool::new(true),
            allow_version: AtomicBool::new(true),
            calls: Mutex::new(Vec::new()),
            absent_barrier: Mutex::new(None),
        }
    }

    pub(crate) fn support(&self, absent: bool, version: bool) {
        self.allow_absent.store(absent, Ordering::SeqCst);
        self.allow_version.store(version, Ordering::SeqCst);
        self.calls.lock().unwrap().clear();
    }

    pub(crate) fn calls(&self) -> Vec<CasExpectation> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl<F: RootFilesystem> RootFilesystem for ConditionalBackend<F> {
    async fn put(
        &self,
        path: &VirtualPath,
        entry: Entry,
        cas: CasExpectation,
    ) -> Result<RecordVersion, FilesystemError> {
        self.calls.lock().unwrap().push(cas);
        if entry.kind.is_some()
            || !entry.indexed.is_empty()
            || (matches!(cas, CasExpectation::Absent) && !self.allow_absent.load(Ordering::SeqCst))
            || (matches!(
                cas,
                CasExpectation::Version(_) | CasExpectation::VersionAndBody { .. }
            ) && !self.allow_version.load(Ordering::SeqCst))
        {
            return Err(FilesystemError::Unsupported {
                path: path.clone(),
                operation: FilesystemOperation::WriteFile,
            });
        }
        let barrier = self.absent_barrier.lock().unwrap().clone();
        if matches!(cas, CasExpectation::Absent)
            && let Some(barrier) = barrier
        {
            barrier.wait().await;
        }
        self.inner.put(path, entry, cas).await
    }

    async fn get(&self, path: &VirtualPath) -> Result<Option<VersionedEntry>, FilesystemError> {
        let selected = self.inner.get(path).await;
        tokio::task::yield_now().await;
        selected
    }

    async fn list_dir(&self, path: &VirtualPath) -> Result<Vec<DirEntry>, FilesystemError> {
        self.inner.list_dir(path).await
    }

    async fn stat(&self, path: &VirtualPath) -> Result<FileStat, FilesystemError> {
        self.inner.stat(path).await
    }
}

pub(crate) fn scoped<F: RootFilesystem>(backend: Arc<F>, alias: &str) -> Arc<ScopedFilesystem<F>> {
    Arc::new(ScopedFilesystem::with_fixed_view(
        backend,
        MountView::new(vec![MountGrant::new(
            MountAlias::new(alias).unwrap(),
            VirtualPath::new(format!("/engine/conditional-owner-contract{alias}")).unwrap(),
            MountPermissions::read_write_list_delete(),
        )])
        .unwrap(),
    ))
}
