//! Original setup readback and completion errors at the actual storage boundary.
use super::*;
use brassclaw_auth::{
    AuthContinuationRef, AuthProviderId, AuthSurface, CredentialAccountLabel,
    CredentialAccountLookupRequest, CredentialAccountService, CredentialAccountStatus,
    CredentialSelectionInput, OAuthCallbackClaimRequest, OAuthCallbackFailureInput,
    OAuthCallbackInput, SecretSubmitRequest, Timestamp,
};
use brassclaw_host_api::{InvocationId, ResourceScope, UserId};
use chrono::Duration;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Readback {
    Original,
    Unavailable,
    Missing,
    ForeignProvider,
    Canceled,
    Expired,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CompletionFault {
    BeforeWrite,
    AfterCommit,
}

/// Delegate actual owner operations. Setup faults lose creation acknowledgements;
/// completion faults reject before writing or lose a real completed write reply.
struct FlowAckLoss<T> {
    inner: Arc<T>,
    mode: Readback,
    completion_fault: Option<CompletionFault>,
    submitted: Mutex<Option<SecretSubmitResult>>,
    creates: AtomicUsize,
    reads: AtomicUsize,
    original: Mutex<Option<AuthFlowRecord>>,
}
impl<T> FlowAckLoss<T> {
    fn new(inner: Arc<T>, mode: Readback) -> Self {
        Self {
            inner,
            mode,
            completion_fault: None,
            submitted: Mutex::new(None),
            creates: AtomicUsize::new(0),
            reads: AtomicUsize::new(0),
            original: Mutex::new(None),
        }
    }
}
#[async_trait]
impl<T: AuthFlowManager + 'static> AuthFlowManager for FlowAckLoss<T> {
    async fn claim_dcr_cleanup(
        &self,
        prepared: brassclaw_auth::DcrCleanupPrepared,
    ) -> Result<brassclaw_auth::DcrCleanupAdmission, brassclaw_auth::AuthProductError> {
        self.inner.claim_dcr_cleanup(prepared).await
    }

    async fn record_dcr_cleanup(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        completion: brassclaw_auth::DcrCleanupCompletion,
    ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError> {
        self.inner.record_dcr_cleanup(scope, completion).await
    }

    async fn update_dcr_retirement(
        &self,
        original: &brassclaw_auth::DcrRegistrationOriginal,
        update: brassclaw_auth::DcrRetirementUpdate,
    ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError> {
        self.inner.update_dcr_retirement(original, update).await
    }

    async fn retain_legacy_dcr(
        &self,
        selection: brassclaw_auth::DcrLegacySelection,
    ) -> Result<brassclaw_auth::DcrLegacyState, brassclaw_auth::AuthProductError> {
        self.inner.retain_legacy_dcr(selection).await
    }

    async fn mark_dcr_cleanup_unavailable(
        &self,
        original: &brassclaw_auth::DcrRegistrationOriginal,
        issue: brassclaw_auth::DcrCleanupUnavailable,
    ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError> {
        self.inner
            .mark_dcr_cleanup_unavailable(original, issue)
            .await
    }

    async fn prepare_oauth_setup(
        &self,
        original: brassclaw_auth::OAuthSetupOriginal,
        retirement: Option<brassclaw_auth::OAuthSetupRetirement>,
    ) -> Result<brassclaw_auth::OAuthSetupState, brassclaw_auth::AuthProductError> {
        self.inner.prepare_oauth_setup(original, retirement).await
    }

    async fn read_oauth_setup(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow_id: brassclaw_auth::AuthFlowId,
    ) -> Result<Option<brassclaw_auth::OAuthSetupState>, brassclaw_auth::AuthProductError> {
        self.inner.read_oauth_setup(scope, flow_id).await
    }

    async fn prepare_dcr_registration(
        &self,
        original: brassclaw_auth::DcrRegistrationOriginal,
    ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError> {
        self.inner.prepare_dcr_registration(original).await
    }

    async fn retire_dcr_preparation(
        &self,
        original: brassclaw_auth::DcrRegistrationOriginal,
        reason: brassclaw_auth::DcrCleanupReason,
    ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError> {
        self.inner.retire_dcr_preparation(original, reason).await
    }

    async fn read_dcr_registration(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        id: brassclaw_auth::AuthFlowId,
    ) -> Result<Option<brassclaw_auth::DcrRegistrationState>, brassclaw_auth::AuthProductError>
    {
        self.inner.read_dcr_registration(scope, id).await
    }

    async fn claim_dcr_registration(
        &self,
        original: &brassclaw_auth::DcrRegistrationOriginal,
    ) -> Result<brassclaw_auth::DcrRegistrationAdmission, brassclaw_auth::AuthProductError> {
        self.inner.claim_dcr_registration(original).await
    }

    async fn record_dcr_registration(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        completion: brassclaw_auth::DcrRegistrationCompletion,
    ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError> {
        self.inner.record_dcr_registration(scope, completion).await
    }

    async fn publish_dcr_challenge(
        &self,
        original: &brassclaw_auth::DcrRegistrationOriginal,
        challenge: brassclaw_auth::DcrRegistrationChallenge,
    ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError> {
        self.inner.publish_dcr_challenge(original, challenge).await
    }

    async fn create_flow(&self, request: NewAuthFlow) -> Result<AuthFlowRecord, AuthProductError> {
        self.creates.fetch_add(1, Ordering::SeqCst);
        assert!(
            request.id.is_some(),
            "identity must be chosen before the write"
        );
        let record = self.inner.create_flow(request).await?;
        *self.original.lock().unwrap() = Some(record.clone());
        if self.mode == Readback::Canceled {
            self.inner.cancel_flow(&record.scope, record.id).await?;
        }
        if self.mode == Readback::Expired {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        if self.completion_fault.is_some() {
            Ok(record)
        } else {
            Err(AuthProductError::BackendUnavailable)
        }
    }
    async fn get_flow(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        {
            let original = self.original.lock().unwrap();
            let original = original.as_ref().unwrap();
            assert_eq!(scope, &original.scope);
            assert_eq!(flow_id, original.id);
        }
        match self.mode {
            Readback::Unavailable => Err(AuthProductError::BackendUnavailable),
            Readback::Missing => Ok(None),
            Readback::ForeignProvider => {
                let mut record = self.inner.get_flow(scope, flow_id).await?.unwrap();
                record.provider = AuthProviderId::new("foreign-provider").unwrap();
                Ok(Some(record))
            }
            _ => self.inner.get_flow(scope, flow_id).await,
        }
    }
    async fn claim_oauth_callback(
        &self,
        scope: &AuthProductScope,
        request: OAuthCallbackClaimRequest,
    ) -> Result<brassclaw_auth::OAuthCallbackAdmission, AuthProductError> {
        self.inner.claim_oauth_callback(scope, request).await
    }
    async fn read_oauth_callback_original(
        &self,
        scope: &AuthProductScope,
        reference: &brassclaw_auth::OAuthCallbackReference,
    ) -> Result<Option<brassclaw_auth::OAuthCallbackState>, AuthProductError> {
        self.inner
            .read_oauth_callback_original(scope, reference)
            .await
    }
    async fn reconcile_oauth_callback(
        &self,
        scope: &AuthProductScope,
        reference: &brassclaw_auth::OAuthCallbackReference,
    ) -> Result<brassclaw_auth::OAuthCallbackState, AuthProductError> {
        self.inner.reconcile_oauth_callback(scope, reference).await
    }
    async fn complete_oauth_callback(
        &self,
        scope: &AuthProductScope,
        input: OAuthCallbackInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.inner.complete_oauth_callback(scope, input).await
    }
    async fn complete_credential_selection(
        &self,
        scope: &AuthProductScope,
        input: CredentialSelectionInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.inner.complete_credential_selection(scope, input).await
    }
    async fn complete_manual_token(
        &self,
        scope: &AuthProductScope,
        input: ManualTokenCompletionInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        if self.completion_fault == Some(CompletionFault::BeforeWrite) {
            return Err(AuthProductError::BackendUnavailable);
        }
        let completed = self.inner.complete_manual_token(scope, input).await?;
        if self.completion_fault == Some(CompletionFault::AfterCommit) {
            Err(AuthProductError::BackendUnavailable)
        } else {
            Ok(completed)
        }
    }
    async fn cancel_manual_token(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        self.inner.cancel_manual_token(scope, interaction_id).await
    }
    async fn fail_oauth_callback(
        &self,
        scope: &AuthProductScope,
        input: OAuthCallbackFailureInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.inner.fail_oauth_callback(scope, input).await
    }
    async fn mark_continuation_dispatched(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
        emitted_at: Timestamp,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.inner
            .mark_continuation_dispatched(scope, flow_id, emitted_at)
            .await
    }
    async fn cancel_flow(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.inner.cancel_flow(scope, flow_id).await
    }
}

#[async_trait]
impl<T: AuthInteractionService + 'static> AuthInteractionService for FlowAckLoss<T> {
    async fn request_secret_input(
        &self,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthChallenge, AuthProductError> {
        self.inner.request_secret_input(request).await
    }
    async fn submit_manual_token(
        &self,
        scope: &AuthProductScope,
        request: SecretSubmitRequest,
    ) -> Result<SecretSubmitResult, AuthProductError> {
        let submitted = self.inner.submit_manual_token(scope, request).await?;
        *self.submitted.lock().unwrap() = Some(submitted.clone());
        Ok(submitted)
    }
    async fn abandon_manual_token(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        self.inner.abandon_manual_token(scope, interaction_id).await
    }
}

async fn exercise_completion<T>(inner: Arc<T>, fault: CompletionFault)
where
    T: AuthFlowManager + AuthInteractionService + CredentialAccountService + 'static,
{
    let mut observed = FlowAckLoss::new(inner.clone(), Readback::Original);
    observed.completion_fault = Some(fault);
    let mut request = request(Readback::Original);
    // This case checks a new account's actual publication. Automatic/bound
    // account selection remains in the separate full manual custody work.
    request.label =
        CredentialAccountLabel::new(format!("completion {}", AuthFlowId::new())).unwrap();
    let owner = request.scope.clone();
    let challenge = request_manual_token_flow_with(&observed, &observed, request)
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired { interaction_id, .. } = challenge else {
        panic!("manual challenge required")
    };
    let original = observed.original.lock().unwrap().clone().unwrap();
    assert_eq!(
        submit_manual_token_flow_with(
            &observed,
            &observed,
            &owner,
            SecretSubmitRequest {
                interaction_id,
                secret: "manual-completion-original-token".into(),
            }
        )
        .await
        .unwrap_err(),
        AuthProductError::BackendUnavailable
    );
    let result = observed.submitted.lock().unwrap().clone().unwrap();
    let account = inner
        .get_account(CredentialAccountLookupRequest::new(
            owner.clone(),
            result.account_id,
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        account.status,
        CredentialAccountStatus::Configured,
        "unknown flow completion cannot revoke the acknowledged account"
    );
    assert!(account.access_secret.is_some());
    let retained = inner.get_flow(&owner, original.id).await.unwrap().unwrap();
    if fault == CompletionFault::AfterCommit {
        assert_eq!(retained.status, AuthFlowStatus::Completed);
        assert_eq!(retained.credential_account_id, Some(result.account_id));
    } else {
        assert_eq!(
            retained, original,
            "a rejected completion cannot cancel the original flow"
        );
    }
}

fn request(mode: Readback) -> ManualTokenSetupRequest {
    ManualTokenSetupRequest {
        requirement_setup: None,
        scope: AuthProductScope::new(
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap(),
            AuthSurface::Web,
        ),
        provider: AuthProviderId::new("github").unwrap(),
        label: CredentialAccountLabel::new("original setup").unwrap(),
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        expires_at: Utc::now()
            + if mode == Readback::Expired {
                Duration::milliseconds(100)
            } else {
                Duration::minutes(5)
            },
    }
}

async fn exercise<T>(inner: Arc<T>, mode: Readback)
where
    T: AuthFlowManager + AuthInteractionService + 'static,
{
    let request = request(mode);
    let owner = request.scope.clone();
    let intended = request.clone();
    let observed = FlowAckLoss::new(inner.clone(), mode);
    let returned = request_manual_token_flow_with(&observed, inner.as_ref(), request).await;
    let original = observed.original.lock().unwrap().clone().unwrap();
    assert_eq!(observed.creates.load(Ordering::SeqCst), 1);
    assert_eq!(observed.reads.load(Ordering::SeqCst), 1);
    let AuthChallenge::ManualTokenRequired { interaction_id, .. } =
        original.challenge.as_ref().unwrap()
    else {
        panic!("actual owner retained the wrong challenge");
    };
    let persisted = inner.get_flow(&owner, original.id).await.unwrap().unwrap();
    assert_eq!(persisted.scope, intended.scope);
    assert_eq!(persisted.provider, intended.provider);
    assert_eq!(persisted.continuation, intended.continuation);
    assert_eq!(persisted.expires_at, intended.expires_at);
    assert_eq!(persisted.update_binding, intended.update_binding);
    match mode {
        Readback::Original => {
            assert_eq!(returned.unwrap(), original.challenge.clone().unwrap());
            assert_eq!(persisted, original);
            // Prove the original interaction was preserved by using its actual
            // first submit, not a fabricated successful replacement challenge.
            let submitted = inner
                .submit_manual_token(
                    &owner,
                    SecretSubmitRequest {
                        interaction_id: *interaction_id,
                        secret: "manual-setup-readback-original-token".into(),
                    },
                )
                .await
                .unwrap();
            assert_eq!(submitted.status, CredentialAccountStatus::Configured);
        }
        Readback::Unavailable | Readback::Missing => {
            assert_eq!(returned.unwrap_err(), AuthProductError::BackendUnavailable);
            assert_eq!(persisted, original);
        }
        Readback::ForeignProvider => {
            assert_eq!(returned.unwrap_err(), AuthProductError::BackendConflict);
            assert_eq!(persisted, original);
        }
        Readback::Canceled => {
            assert_eq!(returned.unwrap_err(), AuthProductError::Canceled);
            assert_eq!(persisted.status, AuthFlowStatus::Canceled);
        }
        Readback::Expired => {
            assert_eq!(
                returned.unwrap_err(),
                AuthProductError::UnknownOrExpiredFlow
            );
            // No cleanup or false completed/canceled state was written.
            assert_eq!(persisted, original);
        }
    }
}

#[tokio::test]
async fn manual_setup_reads_original_without_replacing_or_abandoning_interaction() {
    let inner = Arc::new(InMemoryAuthProductServices::new());
    for mode in [
        Readback::Original,
        Readback::Unavailable,
        Readback::Missing,
        Readback::ForeignProvider,
        Readback::Canceled,
        Readback::Expired,
    ] {
        exercise(inner.clone(), mode).await;
    }
    for fault in [CompletionFault::BeforeWrite, CompletionFault::AfterCommit] {
        exercise_completion(inner.clone(), fault).await;
    }
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pg_manual_setup_recovers_lost_flow_ack_and_retains_unconfirmed_originals() {
    use crate::pg_auth_product_services::{PgAuthProductServices, run_auth_migrations};
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let secrets = Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            "manual-setup-original-master-key-2026".into(),
            "manual-setup",
        )
        .unwrap(),
    );
    let inner = Arc::new(PgAuthProductServices::new(rig.pool.clone(), secrets));
    for mode in [
        Readback::Original,
        Readback::Unavailable,
        Readback::Missing,
        Readback::ForeignProvider,
        Readback::Canceled,
        Readback::Expired,
    ] {
        exercise(inner.clone(), mode).await;
    }
    for fault in [CompletionFault::BeforeWrite, CompletionFault::AfterCommit] {
        exercise_completion(inner.clone(), fault).await;
    }
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fs_manual_setup_recovers_lost_flow_ack_on_actual_persistent_backend() {
    use brassclaw_filesystem::{PostgresRootFilesystem, ScopedFilesystem};
    use brassclaw_host_api::{MountAlias, MountGrant, MountPermissions, MountView, VirtualPath};
    use brassclaw_secrets::{FilesystemSecretStore, SecretsCrypto};
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    root.run_migrations().await.unwrap();
    let mounts = MountView::new(vec![MountGrant::new(
        MountAlias::new("/secrets").unwrap(),
        VirtualPath::new("/tenants/test/users/alice/secrets").unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    let filesystem = Arc::new(ScopedFilesystem::with_fixed_view(root, mounts));
    let crypto =
        Arc::new(SecretsCrypto::new("manual-setup-original-master-key-2026".into()).unwrap());
    let secrets = Arc::new(FilesystemSecretStore::new(filesystem.clone(), crypto));
    let inner = Arc::new(FilesystemAuthProductServices::new(filesystem, secrets));
    for mode in [
        Readback::Original,
        Readback::Unavailable,
        Readback::Missing,
        Readback::ForeignProvider,
        Readback::Canceled,
        Readback::Expired,
    ] {
        exercise(inner.clone(), mode).await;
    }
    for fault in [CompletionFault::BeforeWrite, CompletionFault::AfterCommit] {
        exercise_completion(inner.clone(), fault).await;
    }
}
