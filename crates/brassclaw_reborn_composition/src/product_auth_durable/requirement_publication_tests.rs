//! Real CAS-only Auth owners, including committed-write acknowledgement loss.
use super::*;
use brassclaw_auth::{
    CredentialRequirementSelection, CredentialRequirementSetup, ManualTokenSetupIdentity,
};
use brassclaw_filesystem::{BackendCapabilities, FileStat, VersionedEntry};
use brassclaw_secrets::{FilesystemSecretStore, SecretsCrypto};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy)]
enum Fault {
    StageAck,
    HeadAck,
    FinalAck,
    FinalReject,
    CallbackAccountReject,
}

struct Observed<F> {
    inner: Arc<F>,
    fault: std::sync::Mutex<Option<Fault>>,
    triggered: AtomicUsize,
}

#[async_trait::async_trait]
impl<F: RootFilesystem> RootFilesystem for Observed<F> {
    fn capabilities(&self) -> BackendCapabilities {
        self.inner.capabilities()
    }
    async fn get(&self, path: &VirtualPath) -> Result<Option<VersionedEntry>, FilesystemError> {
        self.inner.get(path).await
    }
    async fn stat(&self, path: &VirtualPath) -> Result<FileStat, FilesystemError> {
        self.inner.stat(path).await
    }
    async fn list_dir(
        &self,
        path: &VirtualPath,
    ) -> Result<Vec<brassclaw_filesystem::DirEntry>, FilesystemError> {
        self.inner.list_dir(path).await
    }
    async fn put(
        &self,
        path: &VirtualPath,
        entry: Entry,
        cas: CasExpectation,
    ) -> Result<RecordVersion, FilesystemError> {
        let fault = {
            let mut selected = self.fault.lock().unwrap();
            let matches = selected.is_some_and(|fault| match fault {
                Fault::HeadAck => path.as_str().contains("/requirement-selections/"),
                Fault::CallbackAccountReject => {
                    path.as_str().contains("/accounts/")
                        && serde_json::from_slice::<serde_json::Value>(&entry.body).is_ok_and(
                            |value| {
                                value["auth_callback_actions"]
                                    .as_object()
                                    .is_some_and(|actions| !actions.is_empty())
                            },
                        )
                }
                other => {
                    serde_json::from_slice::<crate::auth_flow_custody::StoredAuthFlow>(&entry.body)
                        .is_ok_and(|doc| {
                            doc.auth_custody.requirement_selection.is_some()
                                && match other {
                                    Fault::StageAck => {
                                        doc.flow.status == AuthFlowStatus::AwaitingUser
                                            || doc.fs_callback_publication.is_some()
                                    }
                                    Fault::FinalAck | Fault::FinalReject => {
                                        doc.flow.status == AuthFlowStatus::Completed
                                    }
                                    Fault::HeadAck | Fault::CallbackAccountReject => false,
                                }
                        })
                }
            });
            if matches { selected.take() } else { None }
        };
        let failure = || FilesystemError::Backend {
            path: path.clone(),
            operation: brassclaw_filesystem::FilesystemOperation::WriteFile,
            reason: "controlled requirement publication failure".into(),
        };
        if let Some(Fault::FinalReject | Fault::CallbackAccountReject) = fault {
            self.triggered.fetch_add(1, Ordering::SeqCst);
            return Err(failure());
        }
        let version = self.inner.put(path, entry, cas).await?;
        if fault.is_some() {
            self.triggered.fetch_add(1, Ordering::SeqCst);
            return Err(failure());
        }
        Ok(version)
    }
}

#[path = "other_requirement_publication_tests.rs"]
mod other;

fn service<F: RootFilesystem + 'static>(
    root: Arc<Observed<F>>,
) -> FilesystemAuthProductServices<Observed<F>> {
    let mounts = brassclaw_host_api::MountView::new(vec![MountGrant::new(
        MountAlias::new("/secrets").unwrap(),
        VirtualPath::new("/tenants/test/users/alice/secrets").unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    let filesystem = Arc::new(ScopedFilesystem::with_fixed_view(root, mounts));
    let crypto = Arc::new(
        SecretsCrypto::new(SecretString::from(
            "fs-requirement-publication-test-master-key",
        ))
        .unwrap(),
    );
    let secrets = Arc::new(FilesystemSecretStore::new(filesystem.clone(), crypto));
    FilesystemAuthProductServices::new(filesystem, secrets)
}

fn setup(
    scope: &AuthProductScope,
    previous: Option<CredentialRequirementSelection>,
) -> ManualTokenSetupRequest {
    use brassclaw_host_api::{
        NetworkTargetPattern, RuntimeCredentialAccountRequirement, RuntimeCredentialAccountSetup,
        RuntimeCredentialRequirement, RuntimeCredentialRequirementSource, RuntimeCredentialTarget,
    };
    let requirement = RuntimeCredentialAccountRequirement::new(
        ExtensionId::new("fs-requirement-extension").unwrap(),
        RuntimeCredentialRequirement {
            handle: SecretHandle::new("usage-token").unwrap(),
            source: RuntimeCredentialRequirementSource::ProductAuthAccount {
                provider: RuntimeCredentialAccountProviderId::new("manual-provider").unwrap(),
                setup: RuntimeCredentialAccountSetup::ManualToken,
            },
            provider_scopes: vec!["read".into()],
            audience: NetworkTargetPattern {
                scheme: None,
                host_pattern: "api.manual.test".into(),
                port: None,
            },
            target: RuntimeCredentialTarget::Header {
                name: "Authorization".into(),
                prefix: Some("Bearer ".into()),
            },
            required: true,
        },
    )
    .unwrap();
    ManualTokenSetupRequest {
        requirement_setup: Some(Box::new(
            CredentialRequirementSetup::new(scope, requirement, previous).unwrap(),
        )),
        scope: scope.clone(),
        provider: AuthProviderId::new("manual-provider").unwrap(),
        label: CredentialAccountLabel::new("manual requirement").unwrap(),
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        expires_at: Utc::now() + Duration::minutes(5),
    }
}

async fn submit<F: RootFilesystem + 'static>(
    auth: &FilesystemAuthProductServices<F>,
    scope: &AuthProductScope,
    id: ManualTokenSetupIdentity,
    value: &str,
) -> Result<(brassclaw_auth::SecretSubmitResult, Option<AuthFlowRecord>), AuthProductError> {
    auth.submit_manual_owned(
        scope,
        SecretSubmitRequest {
            interaction_id: id.interaction_id,
            secret: value.into(),
        },
        true,
        Some(id.flow_id),
    )
    .await
}

async fn verify<F: RootFilesystem + 'static>(inner: Arc<F>) {
    let root = Arc::new(Observed {
        inner,
        fault: std::sync::Mutex::new(None),
        triggered: AtomicUsize::new(0),
    });
    let auth = service(root.clone());
    let scope = test_scope();
    let mut previous = Some(race(root.clone(), &scope, None).await);
    for fault in [
        Fault::StageAck,
        Fault::HeadAck,
        Fault::FinalAck,
        Fault::FinalReject,
    ] {
        let id = ManualTokenSetupIdentity::default();
        let request = setup(&scope, previous.clone());
        let usage = request
            .requirement_setup
            .as_ref()
            .unwrap()
            .requirement()
            .clone();
        auth.request_fs_manual_setup_original(id, request)
            .await
            .unwrap();
        assert_eq!(
            auth.submit_manual_owned(
                &scope,
                SecretSubmitRequest {
                    interaction_id: id.interaction_id,
                    secret: "bare input".into(),
                },
                false,
                None
            )
            .await
            .unwrap_err(),
            AuthProductError::BackendConflict
        );
        assert!(
            auth.read_manual_original(&scope, id.interaction_id)
                .await
                .unwrap()
                .is_none()
        );
        *root.fault.lock().unwrap() = Some(fault);
        let before = root.triggered.load(Ordering::SeqCst);
        let first = submit(&auth, &scope, id, "original credential").await;
        assert_eq!(root.triggered.load(Ordering::SeqCst), before + 1);
        let reopened = service(root.clone());
        if matches!(fault, Fault::FinalReject) {
            assert_eq!(first.unwrap_err(), AuthProductError::BackendUnavailable);
            let pending = reopened
                .read_flow_document(&scope, id.flow_id)
                .await
                .unwrap()
                .unwrap()
                .0;
            assert_eq!(pending.flow.status, AuthFlowStatus::AwaitingUser);
            assert!(pending.auth_custody.requirement_selection.is_some());
            assert!(pending.flow.continuation_emitted_at.is_none());
            assert_eq!(
                reopened
                    .requirement_selection(&scope, &usage)
                    .await
                    .unwrap_err(),
                AuthProductError::BackendUnavailable
            );
            assert!(
                reopened
                    .continue_manual_original_owned(&scope, id)
                    .await
                    .is_err()
            );
        } else {
            assert_eq!(first.unwrap().1.unwrap().status, AuthFlowStatus::Completed);
        }
        let (result, completed) = submit(&reopened, &scope, id, "original credential")
            .await
            .unwrap();
        assert_eq!(completed.unwrap().status, AuthFlowStatus::Completed);
        let selected = reopened
            .requirement_selection(&scope, &usage)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(selected.original_flow(), id.flow_id);
        let original_source = crate::credential_requirement_custody_tests::published_source(
            &reopened,
            reopened.secret_store.as_ref(),
            &scope,
            &selected,
        )
        .await;
        assert_eq!(selected.account().id, result.account_id);
        assert_eq!(
            selected.account().scopes,
            vec![ProviderScope::new("read").unwrap()]
        );
        let original_result = reopened
            .continue_manual_original_owned(&scope, id)
            .await
            .unwrap();
        // Deliberate successor selection never changes recovery of this original.
        let next_id = ManualTokenSetupIdentity::default();
        let mut next_request = setup(&scope, Some(selected.clone()));
        next_request.update_binding = Some(
            brassclaw_auth::CredentialAccountUpdateBinding::from_projection(
                &selected.account().projection(),
            ),
        );
        reopened
            .request_fs_manual_setup_original(next_id, next_request)
            .await
            .unwrap();
        submit(&reopened, &scope, next_id, "successor credential")
            .await
            .unwrap();
        let latest = reopened
            .requirement_selection(&scope, &usage)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(latest.original_flow(), next_id.flow_id);
        assert_eq!(
            reopened
                .requirement_source_for_flow(&scope, id.flow_id, &usage)
                .await
                .unwrap(),
            Some(original_source)
        );
        assert_eq!(latest.account().id, result.account_id);
        assert_eq!(
            latest.account().scopes,
            vec![ProviderScope::new("read").unwrap()]
        );
        assert_eq!(
            reopened
                .continue_manual_original_owned(&scope, id)
                .await
                .unwrap(),
            original_result
        );
        assert_eq!(
            submit(&reopened, &scope, id, "original credential")
                .await
                .unwrap()
                .0,
            result
        );
        assert!(
            submit(&reopened, &scope, id, "different credential")
                .await
                .is_err()
        );
        assert_eq!(
            reopened
                .requirement_selection(&scope, &usage)
                .await
                .unwrap(),
            Some(latest.clone())
        );
        previous = Some(latest);
    }

    race(root, &scope, previous).await;
}

async fn race<F: RootFilesystem + 'static>(
    root: Arc<Observed<F>>,
    scope: &AuthProductScope,
    previous: Option<CredentialRequirementSelection>,
) -> CredentialRequirementSelection {
    let auth = service(root.clone());
    // Two independent owners capture the same head. The losing new account is
    // retained as a real partial effect; it cannot replace the winning head or
    // produce a successful continuation and is explicitly marked for selection.
    let a = ManualTokenSetupIdentity::default();
    let b = ManualTokenSetupIdentity::default();
    let request = setup(scope, previous);
    let usage = request
        .requirement_setup
        .as_ref()
        .unwrap()
        .requirement()
        .clone();
    auth.request_fs_manual_setup_original(a, request.clone())
        .await
        .unwrap();
    auth.request_fs_manual_setup_original(b, request)
        .await
        .unwrap();
    let other = service(root.clone());
    let (one, two) = tokio::join!(
        submit(&auth, scope, a, "a credential"),
        submit(&other, scope, b, "b credential")
    );
    assert_ne!(one.is_ok(), two.is_ok());
    let (winner, loser, failed) = if one.is_ok() {
        (a, b, two)
    } else {
        (b, a, one)
    };
    assert_eq!(
        failed.unwrap_err(),
        AuthProductError::AccountSelectionRequired
    );
    let current = auth
        .requirement_selection(scope, &usage)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.original_flow(), winner.flow_id);
    let failed_flow = auth
        .read_flow(scope, loser.flow_id)
        .await
        .unwrap()
        .unwrap()
        .0;
    assert_eq!(failed_flow.status, AuthFlowStatus::Failed);
    assert_eq!(
        failed_flow.error,
        Some(brassclaw_auth::AuthErrorCode::AccountSelectionRequired)
    );
    assert!(failed_flow.continuation_emitted_at.is_none());
    let (original, _) = auth
        .read_manual_original(scope, loser.interaction_id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        auth.read_account(scope, original.account.id)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        auth.continue_manual_original_owned(scope, loser)
            .await
            .is_err()
    );
    assert_eq!(
        auth.requirement_selection(scope, &usage).await.unwrap(),
        Some(current.clone())
    );
    current
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_fs_requirement_publication_recovers_originals_and_fences_competitors() {
    verify(Arc::new(InMemoryBackend::new())).await;
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fs_requirement_publication_recovers_originals_and_fences_competitors() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    verify(root).await;
}

// Exercise the actual completed selection reader and runtime selector together.
// This is a dispatch-selection prerequisite, not original-task family capture.
async fn verify_runtime_requirement_selection<F: RootFilesystem + 'static>(inner: Arc<F>) {
    let root = Arc::new(Observed {
        inner,
        fault: std::sync::Mutex::new(None),
        triggered: AtomicUsize::new(0),
    });
    let auth = Arc::new(service(root));
    let scope = test_scope();
    let id = ManualTokenSetupIdentity::default();
    let request = setup(&scope, None);
    let requirement = request
        .requirement_setup
        .as_ref()
        .unwrap()
        .requirement()
        .clone();
    auth.request_fs_manual_setup_original(id, request)
        .await
        .unwrap();
    let (result, completed) = submit(auth.as_ref(), &scope, id, "selected material")
        .await
        .unwrap();
    assert_eq!(completed.unwrap().status, AuthFlowStatus::Completed);
    let selected = auth
        .account_record(&scope, result.account_id)
        .await
        .unwrap()
        .unwrap();
    let other = auth
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: selected.provider.clone(),
            label: CredentialAccountLabel::new("same-provider alternative").unwrap(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: Vec::new(),
            access_secret: Some(SecretHandle::new("unselected-alternative").unwrap()),
            access_expires_at: None,
            refresh_secret: None,
            scopes: selected.scopes.clone(),
        })
        .await
        .unwrap();
    let selector = ProductAuthRuntimeCredentialAccountSelector::new(auth.clone());
    let mut runtime_scope = scope.clone();
    runtime_scope.surface = AuthSurface::Api;
    runtime_scope.resource.invocation_id = InvocationId::new();
    runtime_scope.resource.thread_id = Some(ThreadId::new("runtime-requirement-thread").unwrap());
    let mut owner_scope = runtime_scope.clone();
    owner_scope.resource.thread_id = None;
    let lookup = || {
        RuntimeCredentialAccountSelectionRequest::new(
            CredentialAccountSelectionRequest::new(owner_scope.clone(), selected.provider.clone())
                .for_extension(requirement.requester_extension().clone()),
            runtime_scope.clone(),
            selected.scopes.clone(),
        )
    };
    assert_eq!(
        selector
            .select_unique_configured_runtime_account(lookup())
            .await
            .unwrap_err(),
        AuthProductError::AccountSelectionRequired
    );
    let resolved = selector
        .select_unique_configured_runtime_account(lookup().with_requirement(Some(&requirement)))
        .await
        .unwrap();
    assert_eq!(resolved.id, selected.id);
    assert_ne!(resolved.id, other.id);

    // A broad/native owner lookup never makes a reusable account cross owners.
    for dimension in 0..4 {
        let mut foreign = runtime_scope.clone();
        match dimension {
            0 => {
                foreign.resource.tenant_id =
                    brassclaw_host_api::TenantId::new("foreign-tenant").unwrap()
            }
            1 => foreign.resource.user_id = UserId::new("foreign-user").unwrap(),
            2 => {
                foreign.resource.agent_id =
                    Some(brassclaw_host_api::AgentId::new("foreign-agent").unwrap())
            }
            3 => {
                foreign.resource.project_id =
                    Some(brassclaw_host_api::ProjectId::new("foreign-project").unwrap())
            }
            _ => unreachable!(),
        }
        assert_eq!(
            selector
                .select_unique_configured_runtime_account(
                    RuntimeCredentialAccountSelectionRequest::new(
                        CredentialAccountSelectionRequest::new(
                            owner_scope.clone(),
                            selected.provider.clone()
                        )
                        .for_extension(requirement.requester_extension().clone()),
                        foreign,
                        selected.scopes.clone(),
                    ),
                )
                .await
                .unwrap_err(),
            AuthProductError::CredentialMissing
        );
    }

    // Equal provider/slot/scopes cannot substitute for a different full usage.
    let mut declaration = requirement.requirement().clone();
    declaration.audience.host_pattern = "another.manual.test".into();
    let different = brassclaw_host_api::RuntimeCredentialAccountRequirement::new(
        requirement.requester_extension().clone(),
        declaration,
    )
    .unwrap();
    assert_eq!(
        selector
            .select_unique_configured_runtime_account(lookup().with_requirement(Some(&different)))
            .await
            .unwrap_err(),
        AuthProductError::CredentialMissing
    );

    let resolver =
        ProductAuthRuntimeCredentialResolver::new(Arc::new(selector), auth.secret_store.clone());
    let provider = RuntimeCredentialAccountProviderId::new(selected.provider.as_str()).unwrap();
    let provider_scopes = requirement.requirement().provider_scopes.clone();
    let runtime_request = || RuntimeCredentialAccountRequest {
        requirement: Some(&requirement),
        scope: &runtime_scope.resource,
        provider: &provider,
        provider_scopes: &provider_scopes,
        requester_extension: requirement.requester_extension(),
    };
    let source = resolver
        .resolve_access_secret(runtime_request())
        .await
        .unwrap();
    assert_eq!(Some(source.handle.clone()), selected.access_secret);
    assert_eq!(source.scope, selected.scope.resource);
    assert_eq!(
        Some(source.record_reference.clone()),
        auth.secret_store
            .record_reference(&source.scope, &source.handle)
            .await
            .unwrap()
    );

    auth.update_status(&scope, selected.id, CredentialAccountStatus::Revoked)
        .await
        .unwrap();
    assert_eq!(
        resolver
            .resolve_access_secret(runtime_request())
            .await
            .unwrap_err(),
        brassclaw_host_api::CredentialStageError::AuthRequired
    );
    // The surviving configured alternative must not repair a revoked binding.
    assert_eq!(
        auth.account_record(&scope, other.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        CredentialAccountStatus::Configured
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_fs_runtime_uses_exact_completed_requirement_without_account_substitution() {
    verify_runtime_requirement_selection(Arc::new(InMemoryBackend::new())).await;
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fs_runtime_uses_exact_completed_requirement_without_account_substitution() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    verify_runtime_requirement_selection(root).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archived_v1_fs_original_keeps_its_result_without_inventing_a_binding() {
    let root = Arc::new(Observed {
        inner: Arc::new(InMemoryBackend::new()),
        fault: std::sync::Mutex::new(None),
        triggered: AtomicUsize::new(0),
    });
    let auth = service(root.clone());
    let scope = test_scope();
    let id = ManualTokenSetupIdentity::default();
    let request = setup(&scope, None);
    let usage = request
        .requirement_setup
        .as_ref()
        .unwrap()
        .requirement()
        .clone();
    auth.request_fs_manual_setup_original(id, request)
        .await
        .unwrap();
    let (document, snapshot) = auth
        .read_flow_document(&scope, id.flow_id)
        .await
        .unwrap()
        .unwrap();
    let (pending, _): (
        super::super::interactions::StoredManualTokenInteraction,
        AuthRecordSnapshot,
    ) = auth
        .read_record(
            &scope.resource,
            &super::super::paths::interaction_path(&scope, id.interaction_id),
        )
        .await
        .unwrap()
        .unwrap();
    let prepared = super::super::manual::ManualOriginal::prepare(
        pending,
        super::super::manual::SelectionKind::New,
        true,
        None,
        Some((document, snapshot)),
        Utc::now(),
    )
    .unwrap();
    // Actual encrypted archive-shaped context, not execution of an old image.
    let mut archived = serde_json::to_value(prepared).unwrap();
    archived["kind"] = serde_json::json!("filesystem-manual-token-original/1");
    archived.as_object_mut().unwrap().remove("pending_custody");
    archived["account"]["scopes"] = serde_json::json!([]);
    let legacy: super::super::manual::ManualOriginal =
        serde_json::from_value(archived.clone()).unwrap();
    legacy.verify().unwrap();
    archived["kind"] = serde_json::json!("filesystem-manual-token-original/2");
    let mislabeled: super::super::manual::ManualOriginal =
        serde_json::from_value(archived).unwrap();
    assert!(mislabeled.verify().is_err());
    auth.retain_manual_original(
        &scope,
        SecretSubmitRequest {
            interaction_id: id.interaction_id,
            secret: "legacy original credential".into(),
        },
        legacy,
    )
    .await
    .unwrap();
    let reopened = service(root);
    let (result, completed) = submit(&reopened, &scope, id, "legacy original credential")
        .await
        .unwrap();
    assert_eq!(completed.unwrap().status, AuthFlowStatus::Completed);
    let account = reopened
        .read_account(&scope, result.account_id)
        .await
        .unwrap()
        .unwrap()
        .0;
    assert!(account.scopes.is_empty());
    assert!(
        reopened
            .requirement_selection(&scope, &usage)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        reopened
            .continue_manual_original_owned(&scope, id)
            .await
            .unwrap()
            .0,
        result
    );
    assert!(
        submit(&reopened, &scope, id, "changed legacy credential")
            .await
            .is_err()
    );
}

async fn verify_original_selection_reader<F: RootFilesystem + 'static>(inner: Arc<F>) {
    let root = Arc::new(Observed {
        inner,
        fault: std::sync::Mutex::new(None),
        triggered: AtomicUsize::new(0),
    });
    let first = service(root.clone());
    let reopened = service(root);
    let scope = test_scope();
    let requirement = setup(&scope, None)
        .requirement_setup
        .unwrap()
        .requirement()
        .clone();
    crate::credential_requirement_custody_tests::verify_original_flow_selection(
        &first,
        &reopened,
        scope,
        requirement,
        Some(first.secret_store.as_ref()),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_fs_original_flow_selection_survives_head_change_without_latest_fallback() {
    verify_original_selection_reader(Arc::new(InMemoryBackend::new())).await;
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fs_original_flow_selection_survives_head_change_without_latest_fallback() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    verify_original_selection_reader(root).await;
}
