//! Actual AuthProduct writers against real stores with a controlled CAS window.
use super::*;
use brassclaw_auth::CredentialSetupService;
use brassclaw_filesystem::{
    BackendCapabilities, Capability, DirEntry, FileStat, TxnCapability, VersionedEntry,
};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{Mutex, Notify};

use crate::credential_expiry_test_support as issuer_expiry;

#[tokio::test]
async fn memory_filesystem_original_requirement_custody_rejects_reinterpretation() {
    let filesystem = scoped(Arc::new(InMemoryBackend::new()));
    let secrets = Arc::new(InMemorySecretStore::new());
    let first = FilesystemAuthProductServices::new(filesystem.clone(), secrets.clone());
    let reopened = FilesystemAuthProductServices::new(filesystem, secrets);
    crate::credential_requirement_custody_tests::verify(&first, &reopened, test_scope()).await;
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_filesystem_original_requirement_custody_rejects_reinterpretation() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    let filesystem = scoped(root);
    let secrets = Arc::new(InMemorySecretStore::new());
    let first = FilesystemAuthProductServices::new(filesystem.clone(), secrets.clone());
    let reopened = FilesystemAuthProductServices::new(filesystem, secrets);
    crate::credential_requirement_custody_tests::verify(&first, &reopened, test_scope()).await;
}

#[tokio::test]
async fn memory_filesystem_issuer_expiry_retains_actual_account_snapshot() {
    let auth = FilesystemAuthProductServices::new(
        scoped(Arc::new(InMemoryBackend::new())),
        Arc::new(InMemorySecretStore::new()),
    );
    issuer_expiry::verify(&auth, test_scope()).await;
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_filesystem_issuer_expiry_retains_actual_account_snapshot() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    let auth =
        FilesystemAuthProductServices::new(scoped(root), Arc::new(InMemorySecretStore::new()));
    issuer_expiry::verify(&auth, test_scope()).await;
}

#[derive(Clone, Copy)]
enum PauseAt {
    AnyConditional,
    FinalFlow,
    CallbackAccount,
}

struct PausedAuthRecords<F> {
    inner: Arc<F>,
    at: PauseAt,
    pause: AtomicBool,
    arrived: Notify,
    resume: Notify,
    path: Mutex<Option<VirtualPath>>,
}

#[async_trait::async_trait]
impl<F: RootFilesystem> RootFilesystem for PausedAuthRecords<F> {
    fn capabilities(&self) -> BackendCapabilities {
        let base = BackendCapabilities::empty()
            .with(Capability::Read)
            .with(Capability::Write)
            .with(Capability::Delete)
            .with(Capability::List)
            .with(Capability::Stat)
            .with_txn(TxnCapability::Cas);
        if self.inner.capabilities().has(Capability::EntryBodyCas) {
            base.with(Capability::EntryBodyCas)
        } else {
            base
        }
    }
    async fn put(
        &self,
        path: &VirtualPath,
        entry: Entry,
        cas: CasExpectation,
    ) -> Result<RecordVersion, FilesystemError> {
        let selected = match self.at {
            PauseAt::AnyConditional => true,
            PauseAt::FinalFlow => {
                serde_json::from_slice::<crate::auth_flow_custody::StoredAuthFlow>(&entry.body)
                    .is_ok_and(|doc| doc.flow.status == AuthFlowStatus::Completed)
            }
            PauseAt::CallbackAccount => serde_json::from_slice::<
                super::super::refresh::StoredCredentialAccount,
            >(&entry.body)
            .is_ok_and(|doc| !doc.auth_callback_actions.is_empty()),
        };
        if selected
            && matches!(
                cas,
                CasExpectation::Version(_) | CasExpectation::VersionAndBody { .. }
            )
            && self.pause.swap(false, Ordering::SeqCst)
        {
            *self.path.lock().await = Some(path.clone());
            self.arrived.notify_one();
            self.resume.notified().await;
        }
        self.inner.put(path, entry, cas).await
    }
    async fn get(&self, path: &VirtualPath) -> Result<Option<VersionedEntry>, FilesystemError> {
        self.inner.get(path).await
    }
    async fn delete(&self, path: &VirtualPath) -> Result<(), FilesystemError> {
        self.inner.delete(path).await
    }
    async fn list_dir(&self, path: &VirtualPath) -> Result<Vec<DirEntry>, FilesystemError> {
        self.inner.list_dir(path).await
    }
    async fn stat(&self, path: &VirtualPath) -> Result<FileStat, FilesystemError> {
        self.inner.stat(path).await
    }
}

fn scoped<F: RootFilesystem>(backend: Arc<F>) -> Arc<ScopedFilesystem<F>> {
    let mounts = brassclaw_host_api::MountView::new(vec![MountGrant::new(
        MountAlias::new("/secrets").unwrap(),
        VirtualPath::new("/tenants/test/users/alice/secrets").unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    Arc::new(ScopedFilesystem::with_fixed_view(backend, mounts))
}

fn gate<F: RootFilesystem>(inner: Arc<F>) -> Arc<PausedAuthRecords<F>> {
    Arc::new(PausedAuthRecords {
        inner,
        at: PauseAt::AnyConditional,
        pause: AtomicBool::new(false),
        arrived: Notify::new(),
        resume: Notify::new(),
        path: Mutex::new(None),
    })
}

fn configured(scope: &AuthProductScope) -> NewCredentialAccount {
    NewCredentialAccount {
        scope: scope.clone(),
        provider: google_provider(),
        label: account_label(),
        status: CredentialAccountStatus::Configured,
        ownership: CredentialOwnership::UserReusable,
        owner_extension: None,
        granted_extensions: vec![],
        access_secret: Some(SecretHandle::new("snapshot-original-access").unwrap()),
        access_expires_at: None,
        refresh_secret: None,
        scopes: vec![],
    }
}

async fn verify_account_recreate<F: RootFilesystem + 'static>(first: Arc<F>, second: Arc<F>) {
    let paused = gate(first.clone());
    let secrets = Arc::new(InMemorySecretStore::new());
    let old = FilesystemAuthProductServices::new(scoped(paused.clone()), secrets.clone());
    let other = FilesystemAuthProductServices::new(scoped(second.clone()), secrets);
    let owner = test_scope();
    let original = old.create_account(configured(&owner)).await.unwrap();
    let mut replacement = configured(&owner);
    replacement.access_secret = Some(SecretHandle::new("snapshot-stale-replacement").unwrap());
    paused.pause.store(true, Ordering::SeqCst);
    let writer = old.compare_and_update_account(&original, replacement);
    let racer = async {
        paused.arrived.notified().await;
        let path = paused.path.lock().await.clone().unwrap();
        let before = first.get(&path).await.unwrap().unwrap();
        // The second actual product owner revokes before its real stored record
        // is deleted/recreated, exercising a reused backend version.
        let revoked = other
            .update_status(&owner, original.id, CredentialAccountStatus::Revoked)
            .await
            .unwrap();
        let current = second.get(&path).await.unwrap().unwrap();
        second.delete(&path).await.unwrap();
        let recreated_version = second
            .put(&path, current.entry.clone(), CasExpectation::Absent)
            .await
            .unwrap();
        assert_eq!(
            recreated_version, before.version,
            "must exercise a real reused version"
        );
        assert_ne!(current.entry.body, before.entry.body);
        paused.resume.notify_one();
        revoked
    };
    let (written, revoked) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(writer, racer)
    })
    .await
    .expect("controlled real account recreate must finish");
    assert_eq!(written.unwrap_err(), AuthProductError::BackendConflict);
    assert_eq!(
        other
            .get_account(CredentialAccountLookupRequest::new(owner, original.id))
            .await
            .unwrap(),
        Some(revoked)
    );
}

async fn verify_flow_recreate<F: RootFilesystem + 'static>(first: Arc<F>, second: Arc<F>) {
    let paused = gate(first.clone());
    let secrets = Arc::new(InMemorySecretStore::new());
    let old = FilesystemAuthProductServices::new(scoped(paused.clone()), secrets.clone());
    let other = FilesystemAuthProductServices::new(scoped(second.clone()), secrets);
    let owner = test_scope();
    let flow = fs_callback_flow(&old, &owner, None, "snapshot-flow").await;
    let input = fs_callback_prepared(&flow, "snapshot-flow");
    paused.pause.store(true, Ordering::SeqCst);
    let writer = old.claim_oauth_callback(&owner, input);
    let racer = async {
        paused.arrived.notified().await;
        let path = paused.path.lock().await.clone().unwrap();
        let before = first.get(&path).await.unwrap().unwrap();
        let canceled = other.cancel_flow(&owner, flow.id).await.unwrap();
        let current = second.get(&path).await.unwrap().unwrap();
        second.delete(&path).await.unwrap();
        let recreated_version = second
            .put(&path, current.entry.clone(), CasExpectation::Absent)
            .await
            .unwrap();
        assert_eq!(
            recreated_version, before.version,
            "must exercise a real reused version"
        );
        assert_ne!(current.entry.body, before.entry.body);
        paused.resume.notify_one();
        canceled
    };
    let (admitted, canceled) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(writer, racer)
    })
    .await
    .expect("controlled real flow recreate must finish");
    assert_eq!(admitted.unwrap_err(), AuthProductError::BackendConflict);
    assert_eq!(
        other.get_flow(&owner, flow.id).await.unwrap(),
        Some(canceled)
    );
    assert!(
        other
            .read_oauth_callback_original(
                &owner,
                &fs_callback_prepared(&flow, "snapshot-flow")
                    .original
                    .reference()
            )
            .await
            .unwrap()
            .is_none(),
        "canceled flow must never acquire a replacement dispatch grant"
    );
}

#[tokio::test]
async fn auth_record_snapshots_fence_real_memory_account_and_flow_recreation() {
    let root = Arc::new(InMemoryBackend::new());
    verify_account_recreate(root.clone(), root.clone()).await;
    verify_flow_recreate(root.clone(), root).await;
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_record_snapshots_fence_independent_native_pg_account_and_flow_recreation() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let first = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    let second = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    first.run_migrations().await.unwrap();
    verify_account_recreate(first.clone(), second.clone()).await;
    verify_flow_recreate(first, second).await;
}

#[tokio::test]
async fn auth_record_snapshots_never_downgrade_unsupported_local_or_version_only_writes() {
    let dir = tempfile::tempdir().unwrap();
    let mut root = brassclaw_filesystem::LocalFilesystem::new();
    root.mount_local(
        VirtualPath::new("/tenants/test/users/alice/secrets").unwrap(),
        brassclaw_host_api::HostPath::from_path_buf(dir.path().to_path_buf()),
    )
    .unwrap();
    let auth = FilesystemAuthProductServices::new(
        scoped(Arc::new(root)),
        Arc::new(InMemorySecretStore::new()),
    );
    let owner = test_scope();
    let original = auth.create_account(configured(&owner)).await.unwrap();
    assert_eq!(
        auth.update_status(&owner, original.id, CredentialAccountStatus::Revoked)
            .await
            .unwrap_err(),
        AuthProductError::BackendUnavailable
    );
    let (_, snapshot) = auth
        .read_account(&owner, original.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        auth.write_record(
            &owner.resource,
            &super::super::paths::account_path(&owner, original.id),
            &original,
            CasExpectation::Version(snapshot.version)
        )
        .await
        .unwrap_err(),
        AuthProductError::BackendConflict
    );
    assert_eq!(
        auth.get_account(CredentialAccountLookupRequest::new(owner, original.id))
            .await
            .unwrap(),
        Some(original)
    );
}

async fn verify_legacy_callback_decision<F: RootFilesystem + 'static>(
    root: Arc<F>,
    already_published: bool,
) {
    let paused = Arc::new(PausedAuthRecords {
        inner: root.clone(),
        at: if already_published {
            PauseAt::FinalFlow
        } else {
            PauseAt::CallbackAccount
        },
        pause: AtomicBool::new(false),
        arrived: Notify::new(),
        resume: Notify::new(),
        path: Mutex::new(None),
    });
    let secrets = Arc::new(InMemorySecretStore::new());
    let old = FilesystemAuthProductServices::new(scoped(paused.clone()), secrets.clone());
    let owner = test_scope();
    let original = old.create_account(configured(&owner)).await.unwrap();
    let binding = brassclaw_auth::CredentialAccountUpdateBinding {
        account_id: original.id,
        ownership: original.ownership,
        owner_extension: original.owner_extension.clone(),
        granted_extensions: original.granted_extensions.clone(),
    };
    let flow = fs_callback_flow(&old, &owner, Some(binding), "legacy-snapshot").await;
    let action = fs_callback_dispatch(
        old.claim_oauth_callback(&owner, fs_callback_prepared(&flow, "legacy-snapshot"))
            .await
            .unwrap(),
    );
    paused.pause.store(true, Ordering::SeqCst);
    // Stop the actual owner at the selected storage boundary, then drop that
    // future to model interruption before its next real mutation. No synthetic
    // storage success, duplicate issuer or unconditional CAS is involved.
    {
        let mut writer =
            Box::pin(old.complete_oauth_callback(&owner, fs_callback_completion(&action)));
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            tokio::select! {
                result = &mut writer => panic!("selected publication boundary was not reached: {result:?}"),
                () = paused.arrived.notified() => {}
            }
        }).await.expect("real callback publication must reach the selected boundary");
        drop(writer);
    }
    let restarted = FilesystemAuthProductServices::new(scoped(root.clone()), secrets);
    let flow_path = super::super::paths::flow_path(&owner, flow.id);
    let flow_entry = restarted
        .filesystem
        .get(&owner.resource, &flow_path)
        .await
        .unwrap()
        .unwrap();
    let mut legacy = serde_json::from_slice::<serde_json::Value>(&flow_entry.entry.body).unwrap();
    let decision = legacy["fs_callback_publication"].as_object_mut().unwrap();
    assert!(decision.remove("account_body_checksum").is_some());
    // Write the exact historical JSON shape through the real strong CAS.
    let legacy_entry = Entry::bytes(serde_json::to_vec(&legacy).unwrap());
    restarted
        .filesystem
        .put(
            &owner.resource,
            &flow_path,
            legacy_entry,
            CasExpectation::VersionAndBody {
                version: flow_entry.version,
                body_checksum: EntryBodyChecksum::for_body(&flow_entry.entry.body),
            },
        )
        .await
        .unwrap();
    let (account, snapshot) = restarted
        .read_account_document(&owner, original.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        account.auth_callback_actions.contains_key(&flow.id),
        already_published
    );
    let reference = action.original.reference();
    if already_published {
        let completed = restarted
            .reconcile_oauth_callback(&owner, &reference)
            .await
            .unwrap();
        assert_eq!(
            completed.completed_flow().unwrap().credential_account_id,
            Some(original.id)
        );
        let (after, after_snapshot) = restarted
            .read_account_document(&owner, original.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            snapshot, after_snapshot,
            "existing exact receipt must not rewrite account"
        );
        assert_eq!(
            serde_json::to_value(account).unwrap(),
            serde_json::to_value(after).unwrap()
        );
    } else {
        assert_eq!(
            restarted
                .reconcile_oauth_callback(&owner, &reference)
                .await
                .unwrap_err(),
            AuthProductError::BackendConflict
        );
        let (after, after_snapshot) = restarted
            .read_account_document(&owner, original.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            snapshot, after_snapshot,
            "unproven legacy decision must not publish account"
        );
        assert_eq!(
            serde_json::to_value(account).unwrap(),
            serde_json::to_value(after).unwrap()
        );
        let current = restarted
            .read_flow_document(&owner, flow.id)
            .await
            .unwrap()
            .unwrap()
            .0;
        assert!(
            current.fs_callback_publication.is_some(),
            "unresolved original must remain accountable"
        );
        assert!(
            current
                .fs_callback_publication
                .unwrap()
                .account_body_checksum
                .is_none()
        );
    }
}

#[tokio::test]
async fn auth_record_snapshots_preserve_memory_legacy_receipts_and_unresolved_decisions() {
    let root = Arc::new(InMemoryBackend::new());
    verify_legacy_callback_decision(root.clone(), false).await;
    verify_legacy_callback_decision(root, true).await;
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_record_snapshots_preserve_native_legacy_receipts_and_unresolved_decisions() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    verify_legacy_callback_decision(root.clone(), false).await;
    verify_legacy_callback_decision(root, true).await;
}
