//! Failure scheduling around real encrypted stores; no fake persistence.
use super::*;
use crate::manual_token_flow::RebornManualTokenFlowService;
use brassclaw_auth::SecretSubmitResult;
use brassclaw_filesystem::{
    BackendCapabilities, Capability, DirEntry, EntryBodyChecksum, FileStat, TxnCapability,
    VersionedEntry,
};
use brassclaw_secrets::{FilesystemSecretStore, SecretsCrypto};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::Notify;

#[derive(Clone, Copy, Debug)]
enum Stage {
    SetupInteraction,
    SetupFlow,
    Original,
    Decision,
    Output,
    Account,
    Consumed,
    Flow,
}
#[derive(Clone, Copy)]
enum Fault {
    Before(Stage),
    After(Stage),
    Pause(Stage),
}
impl Fault {
    fn stage(self) -> Stage {
        match self {
            Self::Before(s) | Self::After(s) | Self::Pause(s) => s,
        }
    }
}
fn stage_matches(stage: Stage, entry: &Entry) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&entry.body) else {
        return false;
    };
    let handle = value["handle"].as_str().unwrap_or_default();
    match stage {
        Stage::SetupInteraction => {
            value["setup_flow_id"].is_string()
                && value["publication"].is_null()
                && value["consumed_at"].is_null()
                && value["canceled_at"].is_null()
        }
        Stage::SetupFlow => {
            value["status"] == "awaiting_user"
                && value["challenge"]["type"] == "manual_token_required"
        }
        Stage::Original => handle.starts_with("product-auth-manual-original-"),
        Stage::Output => {
            handle.starts_with("product-auth-manual-")
                && !handle.starts_with("product-auth-manual-original-")
        }
        Stage::Decision => value["publication"]["state"] == "selected",
        Stage::Consumed => value["publication"]["state"] == "published",
        Stage::Account => value["auth_manual_actions"]
            .as_object()
            .is_some_and(|m| !m.is_empty()),
        Stage::Flow => {
            value["status"] == "completed" && value["challenge"]["type"] == "manual_token_required"
        }
    }
}
struct Observed<F> {
    inner: Arc<F>,
    fault: Mutex<Option<Fault>>,
    hits: AtomicUsize,
    account_writes: Arc<AtomicUsize>,
    deletes: Arc<AtomicUsize>,
    arrived: Notify,
    resume: Notify,
}
impl<F> Observed<F> {
    fn new(inner: Arc<F>) -> Self {
        Self {
            inner,
            fault: Mutex::new(None),
            hits: AtomicUsize::new(0),
            account_writes: Arc::new(AtomicUsize::new(0)),
            deletes: Arc::new(AtomicUsize::new(0)),
            arrived: Notify::new(),
            resume: Notify::new(),
        }
    }
}
fn deliberate(path: &VirtualPath) -> FilesystemError {
    FilesystemError::Backend {
        path: path.clone(),
        operation: brassclaw_filesystem::FilesystemOperation::WriteFile,
        reason: "deliberate manual publication acknowledgement/rejection".into(),
    }
}
#[async_trait::async_trait]
impl<F: RootFilesystem> RootFilesystem for Observed<F> {
    fn capabilities(&self) -> BackendCapabilities {
        let base = BackendCapabilities::empty()
            .with(Capability::Read)
            .with(Capability::Write)
            .with(Capability::List)
            .with(Capability::Stat)
            .with(Capability::Delete)
            .with_txn(TxnCapability::Cas);
        let base = if self.inner.capabilities().has(Capability::EntryBodyCas) {
            base.with(Capability::EntryBodyCas)
        } else {
            base
        };
        if self
            .inner
            .capabilities()
            .has(Capability::ConditionalEntryDelete)
        {
            base.with(Capability::ConditionalEntryDelete)
        } else {
            base
        }
    }
    async fn get(&self, path: &VirtualPath) -> Result<Option<VersionedEntry>, FilesystemError> {
        self.inner.get(path).await
    }
    async fn put(
        &self,
        path: &VirtualPath,
        entry: Entry,
        cas: CasExpectation,
    ) -> Result<RecordVersion, FilesystemError> {
        let fault = {
            let mut slot = self.fault.lock().unwrap();
            if slot.is_some_and(|f| stage_matches(f.stage(), &entry)) {
                slot.take()
            } else {
                None
            }
        };
        if let Some(fault) = fault {
            self.hits.fetch_add(1, Ordering::SeqCst);
            match fault {
                Fault::Before(_) => return Err(deliberate(path)),
                Fault::Pause(_) => {
                    self.arrived.notify_one();
                    self.resume.notified().await;
                }
                Fault::After(_) => {}
            }
        }
        let account = stage_matches(Stage::Account, &entry);
        let version = self.inner.put(path, entry, cas).await?;
        if account {
            self.account_writes.fetch_add(1, Ordering::SeqCst);
        }
        if matches!(fault, Some(Fault::After(_))) {
            return Err(deliberate(path));
        }
        Ok(version)
    }
    async fn list_dir(&self, path: &VirtualPath) -> Result<Vec<DirEntry>, FilesystemError> {
        self.inner.list_dir(path).await
    }
    async fn stat(&self, path: &VirtualPath) -> Result<FileStat, FilesystemError> {
        self.inner.stat(path).await
    }
    async fn delete(&self, path: &VirtualPath) -> Result<(), FilesystemError> {
        self.deletes.fetch_add(1, Ordering::SeqCst);
        self.inner.delete(path).await
    }
    async fn delete_if_snapshot(
        &self,
        path: &VirtualPath,
        version: RecordVersion,
        checksum: EntryBodyChecksum,
    ) -> Result<bool, FilesystemError> {
        self.deletes.fetch_add(1, Ordering::SeqCst);
        self.inner.delete_if_snapshot(path, version, checksum).await
    }
}
fn scoped<F: RootFilesystem>(root: Arc<F>) -> Arc<ScopedFilesystem<F>> {
    let mounts = brassclaw_host_api::MountView::new(vec![MountGrant::new(
        MountAlias::new("/secrets").unwrap(),
        VirtualPath::new("/tenants/test/users/alice/secrets").unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    Arc::new(ScopedFilesystem::with_fixed_view(root, mounts))
}
fn crypto() -> Arc<SecretsCrypto> {
    Arc::new(
        SecretsCrypto::new(SecretString::from(
            "fs-manual-original-native-test-master-key-2026",
        ))
        .unwrap(),
    )
}
fn service<F: RootFilesystem + 'static>(
    root: Arc<F>,
    key: Arc<SecretsCrypto>,
) -> (
    Arc<FilesystemAuthProductServices<F>>,
    Arc<FilesystemSecretStore<F>>,
) {
    let filesystem = scoped(root);
    let secrets = Arc::new(FilesystemSecretStore::new(filesystem.clone(), key));
    (
        Arc::new(FilesystemAuthProductServices::new(
            filesystem,
            secrets.clone(),
        )),
        secrets,
    )
}
async fn setup<F: RootFilesystem + 'static>(
    service: &FilesystemAuthProductServices<F>,
    scope: &AuthProductScope,
) -> AuthInteractionId {
    let challenge = service
        .request_manual_token_flow(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: google_provider(),
            label: CredentialAccountLabel::new(format!("manual-{}", AuthInteractionId::new()))
                .unwrap(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired { interaction_id, .. } = challenge else {
        panic!("manual challenge required")
    };
    interaction_id
}
async fn submit<F: RootFilesystem + 'static>(
    service: &FilesystemAuthProductServices<F>,
    scope: &AuthProductScope,
    id: AuthInteractionId,
    raw: &str,
) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
    service
        .submit_manual_token_flow(
            scope,
            SecretSubmitRequest {
                interaction_id: id,
                secret: raw.to_owned().into(),
            },
        )
        .await
}

async fn verify_write_boundaries<F: RootFilesystem + 'static>(
    first: Arc<F>,
    second: Arc<F>,
    after: bool,
) {
    for stage in [
        Stage::Original,
        Stage::Decision,
        Stage::Output,
        Stage::Account,
        Stage::Consumed,
        Stage::Flow,
    ] {
        let observed = Arc::new(Observed::new(first.clone()));
        let key = crypto();
        let (owner, secrets) = service(observed.clone(), key.clone());
        let scope = test_scope();
        let id = setup(&owner, &scope).await;
        *observed.fault.lock().unwrap() = Some(if after {
            Fault::After(stage)
        } else {
            Fault::Before(stage)
        });
        let initial = submit(&owner, &scope, id, "original manual boundary input").await;
        assert_eq!(
            observed.hits.load(Ordering::SeqCst),
            1,
            "the real selected boundary must be exercised: {stage:?}"
        );
        if after {
            assert!(
                initial.is_ok(),
                "known committed original should reconcile: {stage:?}: {initial:?}"
            );
        } else {
            assert!(
                initial.is_err(),
                "rejected publication cannot report success: {stage:?}"
            );
        }
        assert_eq!(
            observed.deletes.load(Ordering::SeqCst),
            0,
            "an error must never trigger compensating deletion"
        );
        let (restarted, reopened_secrets) = service(second.clone(), key);
        let (result, flow) = submit(&restarted, &scope, id, "original manual boundary input")
            .await
            .unwrap();
        assert_eq!(flow.status, AuthFlowStatus::Completed);
        assert_eq!(flow.credential_account_id, Some(result.account_id));
        if let Ok(original) = initial {
            assert_eq!(original, (result.clone(), flow.clone()));
        }
        let account = restarted
            .get_account(CredentialAccountLookupRequest::new(
                scope.clone(),
                result.account_id,
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(
            reopened_secrets
                .matches(
                    &account.scope.resource,
                    account.access_secret.as_ref().unwrap(),
                    &"original manual boundary input".into()
                )
                .await
                .unwrap()
        );
        assert_eq!(
            submit(&restarted, &scope, id, "changed input after boundary")
                .await
                .unwrap_err(),
            AuthProductError::BackendConflict
        );
        let original = SecretHandle::new(format!("product-auth-manual-original-{id}")).unwrap();
        assert!(matches!(
            secrets
                .custody(&scope.resource, &original)
                .await
                .unwrap()
                .unwrap()
                .state,
            brassclaw_secrets::SecretCustodyState::Original { .. }
        ));
        let before = reopened_secrets
            .record_reference(
                &account.scope.resource,
                account.access_secret.as_ref().unwrap(),
            )
            .await
            .unwrap();
        let repeated = submit(&restarted, &scope, id, "original manual boundary input")
            .await
            .unwrap();
        assert_eq!(repeated, (result, flow));
        assert_eq!(
            before,
            reopened_secrets
                .record_reference(
                    &account.scope.resource,
                    account.access_secret.as_ref().unwrap()
                )
                .await
                .unwrap()
        );
    }
}
#[tokio::test]
async fn manual_fs_memory_reconciles_actual_lost_ack_at_every_write() {
    let root = Arc::new(InMemoryBackend::new());
    verify_write_boundaries(root.clone(), root, true).await;
}
#[tokio::test]
async fn manual_fs_memory_rejects_each_write_without_cleanup_or_reselection() {
    let root = Arc::new(InMemoryBackend::new());
    verify_write_boundaries(root.clone(), root, false).await;
}
#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_fs_native_reconciles_actual_lost_ack_at_every_write() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let first = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    first.run_migrations().await.unwrap();
    let second = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    verify_write_boundaries(first, second, true).await;
}
#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_fs_native_rejects_each_write_without_cleanup_or_reselection() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let first = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    first.run_migrations().await.unwrap();
    let second = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    verify_write_boundaries(first, second, false).await;
}

async fn verify_cancellation<F: RootFilesystem + 'static>(first: Arc<F>, second: Arc<F>) {
    for stage in [
        Stage::Decision,
        Stage::Account,
        Stage::Consumed,
        Stage::Flow,
    ] {
        let observed = Arc::new(Observed::new(first.clone()));
        let key = crypto();
        let (owner, _) = service(observed.clone(), key.clone());
        let scope = test_scope();
        let id = setup(&owner, &scope).await;
        let (other, _) = service(second.clone(), key);
        *observed.fault.lock().unwrap() = Some(Fault::Pause(stage));
        let writer = submit(&owner, &scope, id, "cancel handoff original");
        let canceller = async {
            observed.arrived.notified().await;
            let outcome = other.cancel_manual_token(&scope, id).await;
            observed.resume.notify_one();
            outcome
        };
        let (written, canceled) = tokio::time::timeout(std::time::Duration::from_secs(15), async {
            tokio::join!(writer, canceller)
        })
        .await
        .expect("bounded actual cancellation race");
        assert_eq!(observed.hits.load(Ordering::SeqCst), 1);
        let canceled = canceled.unwrap().unwrap();
        if matches!(stage, Stage::Decision) {
            assert_eq!(canceled.status, AuthFlowStatus::Canceled);
            assert_eq!(written.unwrap_err(), AuthProductError::Canceled);
            assert_eq!(observed.account_writes.load(Ordering::SeqCst), 0);
        } else {
            assert_eq!(
                canceled.status,
                AuthFlowStatus::Completed,
                "chosen whole flow must settle before cancellation reply"
            );
            assert_eq!(written.unwrap().1, canceled);
        }
        assert_eq!(observed.deletes.load(Ordering::SeqCst), 0);
    }
}
#[tokio::test]
async fn manual_fs_memory_cancellation_fences_original_decision_and_settles_chosen_writes() {
    let root = Arc::new(InMemoryBackend::new());
    verify_cancellation(root.clone(), root).await;
}
#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_fs_native_cancellation_fences_original_decision_and_settles_chosen_writes() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let first = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    first.run_migrations().await.unwrap();
    let second = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    verify_cancellation(first, second).await;
}

async fn verify_independent_inputs<F: RootFilesystem + 'static>(
    first: Arc<F>,
    second: Arc<F>,
    same: bool,
) {
    let observed = Arc::new(Observed::new(first));
    let key = crypto();
    let (one, _) = service(observed.clone(), key.clone());
    let (two, secrets) = service(second, key);
    let scope = test_scope();
    let id = setup(&one, &scope).await;
    let second_raw = if same {
        "first independent raw"
    } else {
        "second independent raw"
    };
    let (a, b) = tokio::join!(
        submit(&one, &scope, id, "first independent raw"),
        submit(&two, &scope, id, second_raw)
    );
    let winner = match (a, b) {
        (Ok(a), Ok(b)) => {
            assert!(same);
            assert_eq!(a, b);
            a
        }
        (Ok(a), Err(e)) => {
            assert!(!same);
            assert_eq!(e, AuthProductError::BackendConflict);
            a
        }
        (Err(e), Ok(b)) => {
            assert!(!same);
            assert_eq!(e, AuthProductError::BackendConflict);
            b
        }
        other => panic!("one original must publish: {other:?}"),
    };
    let before = secrets
        .custody(
            &scope.resource,
            &SecretHandle::new(format!("product-auth-manual-original-{id}")).unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    let request_raw = if secrets
        .matches(
            &scope.resource,
            &before.handle,
            &"first independent raw".into(),
        )
        .await
        .unwrap()
    {
        "first independent raw"
    } else {
        second_raw
    };
    two.update_status(
        &scope,
        winner.0.account_id,
        CredentialAccountStatus::Revoked,
    )
    .await
    .unwrap();
    let recovered = submit(&two, &scope, id, request_raw).await.unwrap();
    assert_eq!(recovered, winner);
    assert_eq!(
        two.get_account(CredentialAccountLookupRequest::new(
            scope.clone(),
            winner.0.account_id
        ))
        .await
        .unwrap()
        .unwrap()
        .status,
        CredentialAccountStatus::Revoked
    );
    assert_eq!(
        before,
        secrets
            .custody(&scope.resource, &before.handle)
            .await
            .unwrap()
            .unwrap()
    );
}
#[tokio::test]
async fn manual_fs_memory_independent_same_and_changed_inputs_retain_one_original_result() {
    let root = Arc::new(InMemoryBackend::new());
    verify_independent_inputs(root.clone(), root.clone(), true).await;
    verify_independent_inputs(root.clone(), root, false).await;
}
#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_fs_native_independent_same_and_changed_inputs_retain_one_original_result() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let first = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    first.run_migrations().await.unwrap();
    let second = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    verify_independent_inputs(first.clone(), second.clone(), true).await;
    verify_independent_inputs(first, second, false).await;
}

async fn verify_bound_and_legacy_snapshot<F: RootFilesystem + 'static>(root: Arc<F>) {
    let (owner, _) = service(root.clone(), crypto());
    let scope = test_scope();
    let first_id = setup(&owner, &scope).await;
    let (first, _) = submit(&owner, &scope, first_id, "first bound raw")
        .await
        .unwrap();
    let original = owner
        .read_account(&scope, first.account_id)
        .await
        .unwrap()
        .unwrap()
        .0;
    for legacy in [false, true] {
        let before = owner
            .read_account(&scope, original.id)
            .await
            .unwrap()
            .unwrap()
            .0;
        let challenge = owner
            .request_manual_token_flow(ManualTokenSetupRequest {
                requirement_setup: None,
                scope: scope.clone(),
                provider: before.provider.clone(),
                label: before.label.clone(),
                continuation: AuthContinuationRef::SetupOnly,
                update_binding: Some(brassclaw_auth::CredentialAccountUpdateBinding {
                    account_id: before.id,
                    ownership: before.ownership,
                    owner_extension: before.owner_extension.clone(),
                    granted_extensions: before.granted_extensions.clone(),
                }),
                expires_at: Utc::now() + Duration::minutes(5),
            })
            .await
            .unwrap();
        let AuthChallenge::ManualTokenRequired {
            interaction_id: id, ..
        } = challenge
        else {
            panic!("manual challenge")
        };
        if legacy {
            let path = super::super::paths::interaction_path(&scope, id);
            let current = owner
                .filesystem
                .get(&scope.resource, &path)
                .await
                .unwrap()
                .unwrap();
            let mut value: serde_json::Value = serde_json::from_slice(&current.entry.body).unwrap();
            assert!(
                value
                    .as_object_mut()
                    .unwrap()
                    .remove("update_account_snapshot")
                    .is_some()
            );
            owner
                .filesystem
                .put(
                    &scope.resource,
                    &path,
                    Entry::bytes(serde_json::to_vec(&value).unwrap()),
                    CasExpectation::VersionAndBody {
                        version: current.version,
                        body_checksum: EntryBodyChecksum::for_body(&current.entry.body),
                    },
                )
                .await
                .unwrap();
            let (result, _) = submit(&owner, &scope, id, "legacy original flow-bound raw")
                .await
                .unwrap();
            assert_eq!(result.account_id, before.id);
        } else {
            let mut changed = before.clone();
            changed.label = CredentialAccountLabel::new("changed after bound setup").unwrap();
            let (_, snapshot) = owner
                .read_account(&scope, before.id)
                .await
                .unwrap()
                .unwrap();
            owner.write_account(&changed, snapshot.cas()).await.unwrap();
            assert_eq!(
                submit(&owner, &scope, id, "stale bound raw")
                    .await
                    .unwrap_err(),
                AuthProductError::BackendConflict
            );
            assert_eq!(
                owner
                    .get_account(CredentialAccountLookupRequest::new(
                        scope.clone(),
                        before.id
                    ))
                    .await
                    .unwrap(),
                Some(changed)
            );
        }
    }
}
#[tokio::test]
async fn manual_fs_memory_bound_snapshot_and_original_flow_legacy_proof() {
    verify_bound_and_legacy_snapshot(Arc::new(InMemoryBackend::new())).await;
}
#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_fs_native_bound_snapshot_and_original_flow_legacy_proof() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    verify_bound_and_legacy_snapshot(root).await;
}
#[derive(Default)]
struct RejectFirstContinuation(AtomicUsize);
#[async_trait::async_trait]
impl crate::auth::RebornAuthContinuationDispatcher for RejectFirstContinuation {
    async fn dispatch_auth_continuation(
        &self,
        _: brassclaw_auth::AuthContinuationEvent,
    ) -> Result<(), AuthProductError> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            Err(AuthProductError::BackendUnavailable)
        } else {
            Ok(())
        }
    }
}
async fn verify_actual_facade_and_legacy_completion<F: RootFilesystem + 'static>(root: Arc<F>) {
    use crate::auth::{
        RebornManualTokenSetupRequest, RebornManualTokenSubmitRequest,
        RebornProductAuthServicePorts,
    };
    let (owner, _) = service(root, crypto());
    let mut scope = test_scope();
    scope.resource.thread_id = Some(ThreadId::new("fs-manual-facade-thread").unwrap());
    let dispatcher = Arc::new(RejectFirstContinuation::default());
    let facade = RebornProductAuthServicePorts::from_shared_with_provider(
        owner.clone(),
        Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
    )
    .into_services(dispatcher.clone())
    .with_flow_record_source(owner.clone());
    let challenge = facade
        .request_manual_token_setup(RebornManualTokenSetupRequest::new(
            scope.clone(),
            google_provider(),
            account_label(),
            AuthContinuationRef::SetupOnly,
            Utc::now() + Duration::minutes(5),
        ))
        .await
        .unwrap();
    let id = challenge.interaction_id;
    assert!(
        facade
            .submit_manual_token(RebornManualTokenSubmitRequest::new(
                scope.clone(),
                id,
                "facade original raw".into()
            ))
            .await
            .unwrap_err()
            .retryable
    );
    assert!(
        facade
            .submit_manual_token(RebornManualTokenSubmitRequest::new(
                scope.clone(),
                id,
                "facade changed raw".into()
            ))
            .await
            .unwrap_err()
            .conflict
    );
    assert_eq!(dispatcher.0.load(Ordering::SeqCst), 1);
    let result = facade
        .submit_manual_token(RebornManualTokenSubmitRequest::new(
            scope.clone(),
            id,
            "facade original raw".into(),
        ))
        .await
        .unwrap();
    assert_eq!(dispatcher.0.load(Ordering::SeqCst), 2);
    owner
        .update_status(&scope, result.account_id, CredentialAccountStatus::Revoked)
        .await
        .unwrap();
    let repeated = facade
        .submit_manual_token(RebornManualTokenSubmitRequest::new(
            scope.clone(),
            id,
            "facade original raw".into(),
        ))
        .await
        .unwrap();
    assert_eq!(repeated.account_id, result.account_id);
    assert_eq!(repeated.status, result.status);
    assert_eq!(dispatcher.0.load(Ordering::SeqCst), 2);
    assert_eq!(
        owner
            .get_account(CredentialAccountLookupRequest::new(
                scope.clone(),
                result.account_id
            ))
            .await
            .unwrap()
            .unwrap()
            .status,
        CredentialAccountStatus::Revoked
    );
    // Actual old persisted shape: completed manual flow/account but no encrypted
    // original, and its interaction was removed by historical abandonment.
    let legacy = setup(&owner, &scope).await;
    let legacy_account = owner
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: google_provider(),
            label: CredentialAccountLabel::new("legacy configured").unwrap(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: Some(SecretHandle::new("legacy-existing-access").unwrap()),
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![],
        })
        .await
        .unwrap();
    let (mut flow,snapshot)=owner.flows_for_scope(&scope).await.unwrap().into_iter().find(|(flow,_)|
        matches!(&flow.challenge,Some(AuthChallenge::ManualTokenRequired{interaction_id,..}) if *interaction_id==legacy)).unwrap();
    flow.status = AuthFlowStatus::Completed;
    flow.error = None;
    flow.credential_account_id = Some(legacy_account.id);
    flow.updated_at = Utc::now();
    owner
        .write_flow(&scope, &flow, snapshot.cas())
        .await
        .unwrap();
    owner
        .filesystem
        .delete(
            &scope.resource,
            &super::super::paths::interaction_path(&scope, legacy),
        )
        .await
        .unwrap();
    assert!(
        facade
            .submit_manual_token(RebornManualTokenSubmitRequest::new(
                scope.clone(),
                legacy,
                "unproven legacy raw".into()
            ))
            .await
            .unwrap_err()
            .conflict
    );
    assert_eq!(
        dispatcher.0.load(Ordering::SeqCst),
        2,
        "legacy raw input must not trigger latest-account continuation recovery"
    );
    assert_eq!(
        owner
            .complete_manual_token(
                &scope,
                ManualTokenCompletionInput {
                    interaction_id: legacy,
                    credential_account_id: legacy_account.id
                }
            )
            .await
            .unwrap(),
        flow
    );
}
#[tokio::test]
async fn manual_fs_memory_actual_facade_keeps_raw_proof_distinct_from_reference_continuation() {
    verify_actual_facade_and_legacy_completion(Arc::new(InMemoryBackend::new())).await;
}
#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_fs_native_actual_facade_keeps_raw_proof_distinct_from_reference_continuation() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    verify_actual_facade_and_legacy_completion(root).await;
}

async fn verify_original_setup_boundaries<F: RootFilesystem + 'static>(root: Arc<F>) {
    use brassclaw_auth::ManualTokenSetupIdentity;
    for stage in [Stage::SetupInteraction, Stage::SetupFlow] {
        for after in [false, true] {
            let observed = Arc::new(Observed::new(root.clone()));
            let key = crypto();
            let (first, _) = service(observed.clone(), key.clone());
            let scope = test_scope();
            let identity = ManualTokenSetupIdentity::default();
            let request = ManualTokenSetupRequest {
                requirement_setup: None,
                scope: scope.clone(),
                provider: google_provider(),
                label: account_label(),
                continuation: AuthContinuationRef::SetupOnly,
                update_binding: None,
                expires_at: Utc::now() + Duration::minutes(5),
            };
            assert!(
                first
                    .recover_original_manual_token_flow(&scope, identity)
                    .await
                    .unwrap()
                    .is_none()
            );
            *observed.fault.lock().unwrap() = Some(if after {
                Fault::After(stage)
            } else {
                Fault::Before(stage)
            });
            let initial = first
                .request_original_manual_token_flow(identity, request.clone())
                .await;
            assert_eq!(
                observed.hits.load(Ordering::SeqCst),
                1,
                "actual setup write: {stage:?}"
            );
            assert_eq!(initial.is_ok(), after);
            let (restarted, secrets) = service(root.clone(), key);
            let recovered = restarted
                .recover_original_manual_token_flow(&scope, identity)
                .await
                .unwrap();
            let retained = if matches!(stage, Stage::SetupInteraction) && !after {
                assert!(
                    recovered.is_none(),
                    "rejected first write cannot become a new original during recovery"
                );
                restarted
                    .request_original_manual_token_flow(identity, request.clone())
                    .await
                    .unwrap()
            } else {
                recovered.expect("actual retained intent recovers its projection without a caller-supplied binding")
            };
            assert_eq!(retained.id, identity.flow_id);
            assert_eq!(retained.expires_at, request.expires_at);
            let mut retry = request.clone();
            retry.expires_at += Duration::hours(1);
            assert_eq!(
                restarted
                    .request_original_manual_token_flow(identity, retry.clone())
                    .await
                    .unwrap(),
                retained
            );
            let wrong_pair = ManualTokenSetupIdentity {
                flow_id: AuthFlowId::new(),
                ..identity
            };
            assert!(
                restarted
                    .submit_original_manual_token_flow(&scope, wrong_pair, "wrong pair raw".into())
                    .await
                    .is_err()
            );
            assert!(
                secrets
                    .custody(
                        &scope.resource,
                        &SecretHandle::new(format!(
                            "product-auth-manual-original-{}",
                            identity.interaction_id
                        ))
                        .unwrap()
                    )
                    .await
                    .unwrap()
                    .is_none()
            );
            assert!(
                restarted
                    .continue_original_manual_token_flow(&scope, identity)
                    .await
                    .is_err()
            );
            let (result, completed) = restarted
                .submit_original_manual_token_flow(&scope, identity, "  exact original ü  ".into())
                .await
                .unwrap();
            assert_eq!(completed.status, AuthFlowStatus::Completed);
            assert_eq!(completed.id, identity.flow_id);
            assert_eq!(
                restarted
                    .request_original_manual_token_flow(identity, retry)
                    .await
                    .unwrap(),
                completed
            );
            assert!(
                restarted
                    .submit_original_manual_token_flow(&scope, identity, "changed raw".into())
                    .await
                    .is_err()
            );
            assert_eq!(
                restarted
                    .continue_original_manual_token_flow(&scope, identity)
                    .await
                    .unwrap(),
                (result, completed)
            );
            assert_eq!(
                restarted
                    .recover_original_manual_token_flow(&scope, identity)
                    .await
                    .unwrap()
                    .unwrap()
                    .status,
                AuthFlowStatus::Completed
            );
            assert_eq!(observed.deletes.load(Ordering::SeqCst), 0);
        }
    }
}

#[tokio::test]
async fn manual_fs_memory_original_setup_recovers_exact_write_boundaries_and_confirmed_reference() {
    verify_original_setup_boundaries(Arc::new(InMemoryBackend::new())).await;
}
#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_fs_native_original_setup_recovers_exact_write_boundaries_and_confirmed_reference() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    verify_original_setup_boundaries(root).await;
}

async fn verify_original_setup_concurrency_and_cancel<F: RootFilesystem + 'static>(root: Arc<F>) {
    use brassclaw_auth::ManualTokenSetupIdentity;
    let key = crypto();
    let observed = Arc::new(Observed::new(root.clone()));
    let (first, _) = service(observed.clone(), key.clone());
    let (second, _) = service(root.clone(), key.clone());
    let scope = test_scope();
    let identity = ManualTokenSetupIdentity::default();
    let request = ManualTokenSetupRequest {
        requirement_setup: None,
        scope: scope.clone(),
        provider: google_provider(),
        label: account_label(),
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        expires_at: Utc::now() + Duration::minutes(5),
    };
    // Hold the actual initial projection after retaining the interaction. The
    // other owner cancels that same original before any challenge is returned.
    *observed.fault.lock().unwrap() = Some(Fault::Pause(Stage::SetupFlow));
    let task = {
        let first = first.clone();
        let request = request.clone();
        tokio::spawn(async move {
            first
                .request_original_manual_token_flow(identity, request)
                .await
        })
    };
    observed.arrived.notified().await;
    assert!(
        second
            .abandon_manual_token_flow(&scope, identity.interaction_id)
            .await
            .unwrap()
    );
    observed.resume.notify_one();
    let canceled = task.await.unwrap().unwrap();
    assert_eq!(canceled.status, AuthFlowStatus::Canceled);
    let mut later = request.clone();
    later.expires_at += Duration::hours(2);
    assert_eq!(
        second
            .request_original_manual_token_flow(identity, later.clone())
            .await
            .unwrap(),
        canceled
    );
    let (restarted, _) = service(root, key);
    assert_eq!(
        restarted
            .request_original_manual_token_flow(identity, later.clone())
            .await
            .unwrap(),
        canceled
    );
    assert_eq!(
        restarted
            .submit_original_manual_token_flow(&scope, identity, "canceled raw".into())
            .await
            .unwrap_err(),
        AuthProductError::Canceled
    );
    later.label = CredentialAccountLabel::new("changed setup after cancellation").unwrap();
    assert_eq!(
        restarted
            .request_original_manual_token_flow(identity, later)
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    assert_eq!(observed.deletes.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn manual_fs_memory_original_setup_cancellation_during_projection_never_resurrects() {
    verify_original_setup_concurrency_and_cancel(Arc::new(InMemoryBackend::new())).await;
}
#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_fs_native_original_setup_cancellation_during_projection_never_resurrects() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    verify_original_setup_concurrency_and_cancel(root).await;
}

async fn verify_fs_extension_elapsed_expiry<F: RootFilesystem + 'static>(root: Arc<F>) {
    use brassclaw_auth::{CredentialAccountListRequest, ManualTokenSetupIdentity};
    use brassclaw_product_workflow::{
        ExtensionCredentialPrepareRequest, ExtensionCredentialSetupService,
        ExtensionCredentialTarget,
    };
    // Retain a real interaction, then reject the real flow projection write.
    // The original must remain missing that projection until after expiry.
    let observed = Arc::new(Observed::new(root.clone()));
    let key = crypto();
    let (first, _) = service(observed.clone(), key.clone());
    let missing_scope = test_scope();
    let missing = ManualTokenSetupIdentity::default();
    let missing_request = ManualTokenSetupRequest {
        requirement_setup: None,
        scope: missing_scope.clone(),
        provider: google_provider(),
        label: CredentialAccountLabel::new("expiry-extension google token").unwrap(),
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        expires_at: Utc::now() + Duration::seconds(5),
    };
    *observed.fault.lock().unwrap() = Some(Fault::Before(Stage::SetupFlow));
    assert!(
        first
            .request_original_manual_token_flow(missing, missing_request.clone())
            .await
            .is_err()
    );
    assert_eq!(observed.hits.load(Ordering::SeqCst), 1);
    assert!(
        first
            .read_flow_document(&missing_scope, missing.flow_id)
            .await
            .unwrap()
            .is_none()
    );
    let (owner, secrets) = service(root, key);
    let facade = crate::auth::RebornProductAuthServicePorts::from_shared_with_provider(
        owner.clone(),
        Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
    )
    .into_services(Arc::new(RejectFirstContinuation(AtomicUsize::new(1))));
    let (scope, pending, _) = crate::webui_extension_credentials::verify_extension_metadata_expiry(
        facade.clone(),
        test_scope(),
    )
    .await;
    // Both real future timestamps have elapsed; no additional sleep is needed.
    assert!(Utc::now() > missing_request.expires_at);
    assert!(
        owner
            .read_flow_document(&missing_scope, missing.flow_id)
            .await
            .unwrap()
            .is_none()
    );
    let repaired = facade
        .recover_original_manual_token_setup(&missing_scope, missing)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(repaired.id, missing.flow_id);
    assert_eq!(repaired.scope, missing_scope);
    assert_eq!(repaired.status, AuthFlowStatus::AwaitingUser);
    assert_eq!(repaired.expires_at, missing_request.expires_at);
    assert!(
        owner
            .read_flow_document(&missing_scope, missing.flow_id)
            .await
            .unwrap()
            .is_some()
    );
    let adapter = crate::webui_extension_credentials::ProductAuthExtensionCredentialSetup::new(
        Arc::new(facade),
    );
    let projected = adapter
        .prepare_manual_token(ExtensionCredentialPrepareRequest {
            candidate_requirement: None,

            requirement_name: "token".into(),

            scope: missing_scope.clone(),
            requester_extension: brassclaw_host_api::ExtensionId::new("expiry-extension").unwrap(),
            identity: missing,
            target: ExtensionCredentialTarget::New {},
        })
        .await
        .unwrap();
    assert_eq!(projected.status, AuthFlowStatus::Expired);
    assert_eq!(projected.expires_at, missing_request.expires_at);
    let mut extended = missing_request.clone();
    extended.expires_at += Duration::hours(1);
    assert_eq!(
        owner
            .request_original_manual_token_flow(missing, extended)
            .await
            .unwrap_err(),
        AuthProductError::UnknownOrExpiredFlow
    );
    assert_eq!(
        owner
            .submit_original_manual_token_flow(
                &missing_scope,
                missing,
                "expired projection raw".into()
            )
            .await
            .unwrap_err(),
        AuthProductError::UnknownOrExpiredFlow
    );
    assert!(
        owner
            .continue_original_manual_token_flow(&missing_scope, missing)
            .await
            .is_err()
    );
    assert!(
        owner
            .list_accounts(CredentialAccountListRequest::new(
                missing_scope.clone(),
                missing_request.provider
            ))
            .await
            .unwrap()
            .accounts
            .is_empty()
    );
    for (scope, identity) in [(&scope, pending), (&missing_scope, missing)] {
        let original = SecretHandle::new(format!(
            "product-auth-manual-original-{}",
            identity.interaction_id
        ))
        .unwrap();
        assert!(
            secrets
                .custody(&scope.resource, &original)
                .await
                .unwrap()
                .is_none()
        );
        let output = super::super::paths::manual_token_secret_handle(
            CredentialAccountId::from_uuid(identity.interaction_id.as_uuid()),
            identity.interaction_id,
        )
        .unwrap();
        assert!(
            secrets
                .record_reference(&scope.resource, &output)
                .await
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(observed.account_writes.load(Ordering::SeqCst), 0);
    assert_eq!(observed.deletes.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn manual_fs_memory_extension_elapsed_expiry_keeps_metadata_and_effect_fences() {
    verify_fs_extension_elapsed_expiry(Arc::new(InMemoryBackend::new())).await;
}
#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_fs_native_extension_elapsed_expiry_keeps_metadata_and_effect_fences() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    verify_fs_extension_elapsed_expiry(root).await;
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fs_extension_requirement_binding_controls_readiness_and_original_recovery() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    let (native, _) = service(root, crypto());
    let facade = crate::auth::RebornProductAuthServicePorts::from_shared_with_provider(
        native,
        Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
    )
    .into_services(Arc::new(RejectFirstContinuation(AtomicUsize::new(1))));
    crate::webui_extension_credentials::verify_extension_requirement_readiness(
        facade,
        test_scope(),
    )
    .await;
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fs_google_setup_preserves_original_identity_and_verifier() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    let (native, secrets) = service(root, crypto());
    let facade = crate::auth::RebornProductAuthServicePorts::from_shared_with_provider(
        native,
        Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
    )
    .into_services(Arc::new(RejectFirstContinuation(AtomicUsize::new(1))));
    crate::product_auth_serve::verify_google_original_lifecycle(Arc::new(facade), secrets).await;
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fs_dcr_oauth_requirement_handoff_keeps_target_head_and_callback_original() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    let (native, secrets) = service(root, crypto());
    let ports = crate::auth::RebornProductAuthServicePorts::from_shared_with_provider(
        native,
        Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
    );
    crate::product_auth_serve::verify_dcr_oauth_usage(ports, secrets).await;
}
#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fs_dcr_preparation_restores_expired_cancel_and_live_originals_without_replay() {
    use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![
                b"dcr_registration".to_vec(),
                b"not_registered".to_vec(),
                b"expired".to_vec(),
            ],
            vec![],
            AckBoundary::IdleReady,
        )],
    )
    .await
    .unwrap();
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        transport.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    let (native, secrets) = service(root, crypto());
    let ports = crate::auth::RebornProductAuthServicePorts::from_shared_with_provider(
        native,
        Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
    );
    transport.arm(1).unwrap();
    crate::product_auth_serve::verify_dcr_preparation_handoff(
        ports,
        secrets,
        &rig.pool,
        crate::product_auth_serve::DcrPreparationBackend::RecordFilesystem,
    )
    .await;
    assert_eq!(
        transport.cut_count(),
        1,
        "the actual terminal filesystem projection acknowledgement was cut"
    );
    transport.shutdown().await.unwrap();
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fs_oauth_setup_original_keeps_custody_and_reconciles_insert_loss() {
    use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![b"oauth_setup".to_vec(), b"expired".to_vec()],
            vec![],
            AckBoundary::IdleReady,
        )],
    )
    .await
    .unwrap();
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        transport.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    let key = crypto();
    let (first, secrets) = service(root.clone(), key.clone());
    let (second, _) = service(root, key);
    transport.arm(1).unwrap();
    crate::auth_oauth_setup_custody::tests::verify_storage(
        first.clone(),
        second,
        first,
        secrets,
        &rig.pool,
        crate::auth_oauth_setup_custody::tests::Backend::RecordFilesystem,
        test_scope(),
    )
    .await;
    assert_eq!(
        transport.cut_count(),
        1,
        "actual expired setup filesystem write acknowledgement was cut"
    );
    transport.shutdown().await.unwrap();
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fs_google_private_client_and_pkce_survive_restart_callback_and_refresh() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    let key = crypto();
    let (first, first_secrets) = service(root.clone(), key.clone());
    let (second, second_secrets) = service(root, key);
    crate::oauth_gate::verify_native_private_runtime(
        crate::auth::RebornProductAuthServicePorts::from_shared_with_provider(
            first,
            Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
        ),
        crate::auth::RebornProductAuthServicePorts::from_shared_with_provider(
            second,
            Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
        ),
        first_secrets,
        second_secrets,
        &rig.pool,
        crate::auth_oauth_setup_custody::tests::Backend::RecordFilesystem,
        test_scope(),
    )
    .await;
}
