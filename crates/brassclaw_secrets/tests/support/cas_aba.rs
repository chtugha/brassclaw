//! Controlled scheduling around real stores; every read/write uses the backend.
use super::{FilesystemSecretStore, ResourceScope, SecretHandle, SecretStore, SecretsCrypto};
use async_trait::async_trait;
use brassclaw_filesystem::{
    BackendCapabilities, Capability, CasExpectation, DirEntry, Entry, FileStat, FilesystemError,
    IndexSpec, RecordVersion, RootFilesystem, ScopedFilesystem, TxnCapability, VersionedEntry,
};
use brassclaw_host_api::{MountAlias, MountGrant, MountPermissions, MountView, VirtualPath};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{Mutex, Notify};

struct PausedBackend<F> {
    inner: Arc<F>,
    pause: AtomicBool,
    arrived: Notify,
    resume: Notify,
    path: Mutex<Option<VirtualPath>>,
}
#[async_trait]
impl<F: RootFilesystem> RootFilesystem for PausedBackend<F> {
    fn capabilities(&self) -> BackendCapabilities {
        let capabilities = BackendCapabilities::empty()
            .with(Capability::Read)
            .with(Capability::Write)
            .with(Capability::List)
            .with(Capability::Stat)
            .with(Capability::Delete)
            .with(Capability::Records)
            .with(Capability::IndexExact)
            .with(Capability::EntryBodyCas)
            .with_txn(TxnCapability::Cas);
        if self
            .inner
            .capabilities()
            .has(Capability::ConditionalEntryDelete)
        {
            capabilities.with(Capability::ConditionalEntryDelete)
        } else {
            capabilities
        }
    }
    async fn put(
        &self,
        path: &VirtualPath,
        entry: Entry,
        cas: CasExpectation,
    ) -> Result<RecordVersion, FilesystemError> {
        let pauseable = matches!(
            cas,
            CasExpectation::Version(_) | CasExpectation::VersionAndBody { .. }
        ) || (cas == CasExpectation::Absent
            && path.as_str().contains("/secret-leases/"));
        if pauseable && self.pause.swap(false, Ordering::SeqCst) {
            *self.path.lock().await = Some(path.clone());
            self.arrived.notify_one();
            self.resume.notified().await;
        }
        self.inner.put(path, entry, cas).await
    }
    async fn get(&self, path: &VirtualPath) -> Result<Option<VersionedEntry>, FilesystemError> {
        self.inner.get(path).await
    }
    async fn delete_if_snapshot(
        &self,
        path: &VirtualPath,
        version: RecordVersion,
        body_checksum: brassclaw_filesystem::EntryBodyChecksum,
    ) -> Result<bool, FilesystemError> {
        if self.pause.swap(false, Ordering::SeqCst) {
            *self.path.lock().await = Some(path.clone());
            self.arrived.notify_one();
            self.resume.notified().await;
        }
        self.inner
            .delete_if_snapshot(path, version, body_checksum)
            .await
    }
    async fn delete(&self, path: &VirtualPath) -> Result<(), FilesystemError> {
        self.inner.delete(path).await
    }
    async fn ensure_index(
        &self,
        path: &VirtualPath,
        spec: &IndexSpec,
    ) -> Result<(), FilesystemError> {
        self.inner.ensure_index(path, spec).await
    }
    async fn list_dir(&self, path: &VirtualPath) -> Result<Vec<DirEntry>, FilesystemError> {
        self.inner.list_dir(path).await
    }
    async fn stat(&self, path: &VirtualPath) -> Result<FileStat, FilesystemError> {
        self.inner.stat(path).await
    }
}
fn scoped<F: RootFilesystem>(backend: Arc<F>, scope: &ResourceScope) -> Arc<ScopedFilesystem<F>> {
    let mounts = MountView::new(vec![MountGrant::new(
        MountAlias::new("/secrets").unwrap(),
        VirtualPath::new(format!(
            "/tenants/{}/users/{}/secrets",
            scope.tenant_id, scope.user_id
        ))
        .unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    Arc::new(ScopedFilesystem::with_fixed_view(backend, mounts))
}

pub(crate) async fn verify_lease_consumption_aba<F: RootFilesystem + 'static>(
    first: Arc<F>,
    second: Arc<F>,
    scope: ResourceScope,
    crypto: Arc<SecretsCrypto>,
) {
    let gate = Arc::new(PausedBackend {
        inner: first.clone(),
        pause: AtomicBool::new(false),
        arrived: Notify::new(),
        resume: Notify::new(),
        path: Mutex::new(None),
    });
    let original = FilesystemSecretStore::new(scoped(gate.clone(), &scope), crypto.clone());
    let other = FilesystemSecretStore::new(scoped(second.clone(), &scope), crypto);
    let handle = SecretHandle::new("lease-aba-selected-material").unwrap();
    original
        .put(
            scope.clone(),
            handle.clone(),
            "selected before revoked lease restore".into(),
        )
        .await
        .unwrap();
    let lease = original.lease_once(&scope, &handle).await.unwrap();
    let alias = brassclaw_host_api::ScopedPath::new(format!(
        "/secrets/{}/secret-leases/{}.json",
        scope.within_agent_project_segment(),
        lease.id
    ))
    .unwrap();
    let view = scoped(first.clone(), &scope);
    let active = view.get(&scope, &alias).await.unwrap().unwrap();
    let path = active.path.clone();
    other.revoke(&scope, lease.id).await.unwrap();
    let revoked = second.get(&path).await.unwrap().unwrap();
    // Both bodies came from the real lease issuer/revoker and retain valid
    // authenticated selection proofs. The raw fixture models cross-process
    // delete/restore without relying on the process-local lease mutex.
    first.delete(&path).await.unwrap();
    first
        .put(&path, active.entry, CasExpectation::Absent)
        .await
        .unwrap();
    gate.pause.store(true, Ordering::SeqCst);
    let consume = original.consume(&scope, lease.id);
    let restore = async {
        gate.arrived.notified().await;
        let selected = first.get(&path).await.unwrap().unwrap();
        assert_eq!(gate.path.lock().await.as_ref(), Some(&path));
        second.delete(&path).await.unwrap();
        second
            .put(&path, revoked.entry.clone(), CasExpectation::Absent)
            .await
            .unwrap();
        let actual = second.get(&path).await.unwrap().unwrap();
        assert_eq!(
            selected.version, actual.version,
            "exercise the actual reused backend version"
        );
        assert_ne!(selected.entry.body, actual.entry.body);
        gate.resume.notify_one();
    };
    let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(consume, restore)
    })
    .await
    .expect("real lease CAS/revocation barrier completes");
    assert!(
        result.is_err(),
        "a stale consume cannot overwrite a restored revoked lease or release material"
    );
    assert_eq!(
        second.get(&path).await.unwrap().unwrap().entry.body,
        revoked.entry.body
    );
}

pub(crate) async fn verify_secret_aba<F: RootFilesystem + 'static>(
    first: Arc<F>,
    second: Arc<F>,
    scope: ResourceScope,
    crypto: Arc<SecretsCrypto>,
) {
    verify_atomic_body_cas(first.as_ref()).await;
    assert!(first.capabilities().has(Capability::EntryBodyCas));
    assert!(second.capabilities().has(Capability::EntryBodyCas));
    let gate = Arc::new(PausedBackend {
        inner: first.clone(),
        pause: AtomicBool::new(true),
        arrived: Notify::new(),
        resume: Notify::new(),
        path: Mutex::new(None),
    });
    let old = FilesystemSecretStore::new(scoped(gate.clone(), &scope), crypto.clone());
    let other = FilesystemSecretStore::new(scoped(second.clone(), &scope), crypto);
    let handle = SecretHandle::new("secret-aba-window").unwrap();
    old.put(scope.clone(), handle.clone(), "original".into())
        .await
        .unwrap();
    let original = old
        .record_reference(&scope, &handle)
        .await
        .unwrap()
        .unwrap();
    let writer = old.put_if_record_reference(
        scope.clone(),
        handle.clone(),
        "stale-replacement".into(),
        Some(&original),
    );
    let racer = async {
        gate.arrived.notified().await;
        let path = gate.path.lock().await.clone().unwrap();
        let before = first.get(&path).await.unwrap().unwrap();
        other.delete(&scope, &handle).await.unwrap();
        other
            .put(scope.clone(), handle.clone(), "new-owner-value".into())
            .await
            .unwrap();
        let after = second.get(&path).await.unwrap().unwrap();
        assert_eq!(
            before.version, after.version,
            "real backend must exercise reused version"
        );
        assert_ne!(before.entry.body, after.entry.body);
        gate.resume.notify_one();
    };
    let (written, ()) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(writer, racer)
    })
    .await
    .expect("controlled real-owner race must finish");
    assert!(
        written.unwrap().is_none(),
        "stale conditional write must not overwrite recreated secret"
    );
    assert!(
        other
            .matches(&scope, &handle, &"new-owner-value".into())
            .await
            .unwrap()
    );
    assert_ne!(
        other.record_reference(&scope, &handle).await.unwrap(),
        Some(original)
    );
}

async fn verify_atomic_body_cas<F: RootFilesystem>(backend: &F) {
    use brassclaw_filesystem::{EntryBodyChecksum, FilesystemOperation};
    let path = VirtualPath::new("/tenants/cas-body-contract/entry").unwrap();
    let version = backend
        .put(
            &path,
            Entry::bytes(b"first opaque body".to_vec()),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
    let stale = CasExpectation::VersionAndBody {
        version,
        body_checksum: EntryBodyChecksum::for_body(b"first opaque body"),
    };
    backend.delete(&path).await.unwrap();
    assert_eq!(
        backend
            .put(
                &path,
                Entry::bytes(b"second opaque body".to_vec()),
                CasExpectation::Absent
            )
            .await
            .unwrap(),
        version
    );
    let error = backend
        .put(&path, Entry::bytes(b"must not write".to_vec()), stale)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        FilesystemError::SnapshotMismatch {
            operation: FilesystemOperation::WriteFile,
            ..
        }
    ));
    assert_eq!(
        backend.get(&path).await.unwrap().unwrap().entry.body,
        b"second opaque body"
    );
    let selected = backend.get(&path).await.unwrap().unwrap();
    let next = backend
        .put(
            &path,
            Entry::bytes(b"accepted successor".to_vec()),
            CasExpectation::VersionAndBody {
                version: selected.version,
                body_checksum: EntryBodyChecksum::for_body(&selected.entry.body),
            },
        )
        .await
        .unwrap();
    assert_ne!(next, version);
    assert!(matches!(
        backend
            .put(&path, Entry::bytes(b"stale version".to_vec()), stale)
            .await
            .unwrap_err(),
        FilesystemError::VersionMismatch { .. }
    ));
    backend.delete(&path).await.unwrap();
    assert!(matches!(
        backend
            .put(&path, Entry::bytes(b"must remain absent".to_vec()), stale)
            .await
            .unwrap_err(),
        FilesystemError::VersionMismatch { found: None, .. }
    ));
    assert!(backend.get(&path).await.unwrap().is_none());
}

pub(crate) async fn verify_secret_delete_aba<F: RootFilesystem + 'static>(
    first: Arc<F>,
    second: Arc<F>,
    scope: ResourceScope,
    crypto: Arc<SecretsCrypto>,
) {
    assert!(first.capabilities().has(Capability::ConditionalEntryDelete));
    assert!(
        second
            .capabilities()
            .has(Capability::ConditionalEntryDelete)
    );
    let gate = Arc::new(PausedBackend {
        inner: first.clone(),
        pause: AtomicBool::new(true),
        arrived: Notify::new(),
        resume: Notify::new(),
        path: Mutex::new(None),
    });
    let old = FilesystemSecretStore::new(scoped(gate.clone(), &scope), crypto.clone());
    let other = FilesystemSecretStore::new(scoped(second.clone(), &scope), crypto);
    let handle = SecretHandle::new("custody-delete-recreate-window").unwrap();
    old.put(scope.clone(), handle.clone(), "old ordinary value".into())
        .await
        .unwrap();
    let deletion = old.delete(&scope, &handle);
    let racer = async {
        gate.arrived.notified().await;
        let path = gate.path.lock().await.clone().unwrap();
        let before = first.get(&path).await.unwrap().unwrap();
        assert!(other.delete(&scope, &handle).await.unwrap());
        let replacement = other
            .put_original(
                scope.clone(),
                handle.clone(),
                "new immutable original".into(),
                serde_json::json!({"selection":"actual replacement"}),
            )
            .await
            .unwrap()
            .unwrap();
        let after = second.get(&path).await.unwrap().unwrap();
        assert_eq!(
            before.version, after.version,
            "exercise actual backend version reuse"
        );
        assert_ne!(before.entry.body, after.entry.body);
        gate.resume.notify_one();
        replacement
    };
    let (removed, replacement) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(deletion, racer)
    })
    .await
    .expect("controlled actual ordinary-delete/original-insert race must finish");
    assert!(
        !removed.unwrap(),
        "stale ordinary deletion must never remove a new immutable original"
    );
    assert_eq!(
        other.custody(&scope, &handle).await.unwrap(),
        Some(replacement)
    );
    assert!(
        other
            .matches(&scope, &handle, &"new immutable original".into())
            .await
            .unwrap()
    );
    assert!(other.lease_once(&scope, &handle).await.is_err());
}

pub(crate) async fn verify_conditional_lease_issuance<F: RootFilesystem + 'static>(
    first: Arc<F>,
    second: Arc<F>,
    scope: ResourceScope,
    crypto: Arc<SecretsCrypto>,
) {
    let gate = Arc::new(PausedBackend {
        inner: first,
        pause: AtomicBool::new(false),
        arrived: Notify::new(),
        resume: Notify::new(),
        path: Mutex::new(None),
    });
    let selected = FilesystemSecretStore::new(scoped(gate.clone(), &scope), crypto.clone());
    let writer = FilesystemSecretStore::new(scoped(second, &scope), crypto);
    let handle = SecretHandle::new("lease-issuance-selected-generation").unwrap();
    selected
        .put(
            scope.clone(),
            handle.clone(),
            "selected before lease write".into(),
        )
        .await
        .unwrap();
    let reference = selected
        .record_reference(&scope, &handle)
        .await
        .unwrap()
        .unwrap();
    gate.pause.store(true, Ordering::SeqCst);
    let issue = selected.lease_once_if_record_reference(&scope, &handle, &reference);
    let replace = async {
        gate.arrived.notified().await;
        assert!(
            gate.path
                .lock()
                .await
                .as_ref()
                .unwrap()
                .as_str()
                .contains("/secret-leases/")
        );
        writer
            .put(
                scope.clone(),
                handle.clone(),
                "replacement during lease write".into(),
            )
            .await
            .unwrap();
        gate.resume.notify_one();
    };
    let (lease, ()) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(issue, replace)
    })
    .await
    .expect("actual conditional lease write barrier completes");
    let lease = lease.unwrap().unwrap();
    assert!(
        selected.consume(&scope, lease.id).await.is_err(),
        "a lease selected before the wait cannot release replacement material"
    );
    assert!(
        selected
            .lease_once_if_record_reference(&scope, &handle, &reference)
            .await
            .unwrap()
            .is_none()
    );
    let current = writer
        .record_reference(&scope, &handle)
        .await
        .unwrap()
        .unwrap();
    let lease = writer
        .lease_once_if_record_reference(&scope, &handle, &current)
        .await
        .unwrap()
        .unwrap();
    let material = writer.consume(&scope, lease.id).await.unwrap();
    use secrecy::ExposeSecret;
    assert_eq!(material.expose_secret(), "replacement during lease write");
}
