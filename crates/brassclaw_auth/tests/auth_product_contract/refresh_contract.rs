use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use crate::common::*;
use async_trait::async_trait;
use brassclaw_auth::{OAuthProviderRefresh, ProviderBackedCredentialAccountService};
use tokio::sync::Notify;

struct BlockingRefreshProvider {
    inner: Arc<InMemoryAuthProductServices>,
    refresh_calls: AtomicUsize,
    refresh_started: Notify,
    release_refresh: Notify,
    refresh_released: AtomicBool,
}

impl BlockingRefreshProvider {
    fn new(inner: Arc<InMemoryAuthProductServices>) -> Self {
        Self {
            inner,
            refresh_calls: AtomicUsize::new(0),
            refresh_started: Notify::new(),
            release_refresh: Notify::new(),
            refresh_released: AtomicBool::new(false),
        }
    }

    fn refresh_call_count(&self) -> usize {
        self.refresh_calls.load(Ordering::SeqCst)
    }

    async fn wait_for_first_refresh_start(&self) {
        self.refresh_started.notified().await;
    }

    fn release_refresh(&self) {
        self.refresh_released.store(true, Ordering::SeqCst);
        self.release_refresh.notify_waiters();
    }
}

fn provider_backed_auth(
    services: Arc<InMemoryAuthProductServices>,
) -> Arc<ProviderBackedCredentialAccountService> {
    Arc::new(ProviderBackedCredentialAccountService::new(
        services.clone(),
        services.clone(),
        services,
    ))
}

#[async_trait]
impl AuthProviderClient for BlockingRefreshProvider {
    async fn prepare_callback(
        &self,
        context: &brassclaw_auth::OAuthProviderExchangeContext,
        request: &brassclaw_auth::OAuthProviderCallbackRequest,
    ) -> Result<brassclaw_auth::OAuthTokenDestinations, AuthProductError> {
        self.inner.prepare_callback(context, request).await
    }

    async fn prepare_refresh(
        &self,
        request: &OAuthProviderRefreshRequest,
    ) -> Result<brassclaw_auth::OAuthProviderRefreshPreparation, AuthProductError> {
        self.inner.prepare_refresh(request).await
    }

    async fn exchange_callback(
        &self,
        context: brassclaw_auth::OAuthProviderExchangeContext,
        request: brassclaw_auth::OAuthProviderCallbackRequest,
    ) -> Result<brassclaw_auth::OAuthProviderExchange, AuthProductError> {
        self.inner.exchange_callback(context, request).await
    }

    async fn refresh_token(
        &self,
        request: OAuthProviderRefreshRequest,
        preparation: brassclaw_auth::OAuthProviderRefreshPreparation,
    ) -> Result<OAuthProviderRefresh, AuthProductError> {
        self.refresh_calls.fetch_add(1, Ordering::SeqCst);
        self.refresh_started.notify_one();
        loop {
            let notified = self.release_refresh.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.refresh_released.load(Ordering::SeqCst) {
                break;
            }
            notified.await;
        }
        self.inner.refresh_token(request, preparation).await
    }
}

#[tokio::test]
async fn credential_refresh_updates_account_through_provider_boundary() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let old_access = SecretHandle::new("github-refresh-old-access").unwrap();
    let old_refresh = SecretHandle::new("github-refresh-old-refresh").unwrap();
    let account = services
        .create_account(NewCredentialAccount {
            scope: owner.clone(),
            provider: provider(),
            label: label("work"),
            status: CredentialAccountStatus::Expired,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: Vec::new(),
            access_secret: Some(old_access.clone()),
            access_expires_at: None,
            refresh_secret: Some(old_refresh.clone()),
            scopes: provider_scopes(&["repo"]),
        })
        .await
        .expect("expired account");

    let report = services
        .refresh_account(CredentialRefreshRequest::new(
            owner.clone(),
            provider(),
            account.id,
        ))
        .await
        .expect("refresh account");

    assert!(report.refreshed);
    assert_eq!(report.account.id, account.id);
    assert_eq!(report.account.status, CredentialAccountStatus::Configured);
    assert_eq!(report.recovery.kind(), CredentialRecoveryKind::Configured);
    assert_eq!(
        report.recovery.selected_account().map(|account| account.id),
        Some(account.id)
    );

    let refreshed = services
        .get_account(CredentialAccountLookupRequest::new(
            owner.clone(),
            account.id,
        ))
        .await
        .expect("lookup")
        .expect("refreshed account");
    assert_eq!(refreshed.status, CredentialAccountStatus::Configured);
    assert_ne!(refreshed.access_secret, Some(old_access));
    assert_ne!(refreshed.refresh_secret, Some(old_refresh));
    assert_eq!(refreshed.scopes, provider_scopes(&["repo"]));

    let serialized = serde_json::to_string(&report).expect("serialize report");
    assert!(!serialized.contains("github-refresh-old-access"));
    assert!(!serialized.contains("github-refresh-old-refresh"));
    assert!(!serialized.contains("oauth-refreshed"));
    assert!(!serialized.contains("RAW_PROVIDER_ERROR_SENTINEL"));
}

#[tokio::test]
async fn credential_refresh_failure_becomes_recoverable_status() {
    let services = Arc::new(InMemoryAuthProductServices::new());
    let auth = provider_backed_auth(services.clone());
    let owner = scope("alice");
    let account = auth
        .create_account(NewCredentialAccount {
            scope: owner.clone(),
            provider: provider(),
            label: label("work"),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: Vec::new(),
            access_secret: Some(SecretHandle::new("github-refresh-fail-access").unwrap()),
            access_expires_at: None,
            refresh_secret: Some(SecretHandle::new("github-refresh-fail-refresh").unwrap()),
            scopes: provider_scopes(&["repo"]),
        })
        .await
        .expect("configured account");
    services.fail_next_refresh_for_tests(account.id);

    let report = auth
        .refresh_account(CredentialRefreshRequest::new(
            owner.clone(),
            provider(),
            account.id,
        ))
        .await
        .expect("refresh failure is projected");

    assert!(!report.refreshed);
    assert_eq!(
        report.account.status,
        CredentialAccountStatus::RefreshFailed
    );
    assert_eq!(
        report.recovery.kind(),
        CredentialRecoveryKind::ReauthorizeRequired
    );
    assert_eq!(
        report.recovery.reason,
        CredentialRecoveryReason::RefreshFailed
    );
    assert_eq!(report.recovery.choices().len(), 1);
    assert_eq!(report.recovery.choices()[0].id, account.id);

    let failed = services
        .get_account(CredentialAccountLookupRequest::new(
            owner.clone(),
            account.id,
        ))
        .await
        .expect("lookup")
        .expect("failed account");
    assert_eq!(failed.status, CredentialAccountStatus::RefreshFailed);

    let serialized = serde_json::to_string(&report).expect("serialize report");
    assert!(!serialized.contains("github-refresh-fail-access"));
    assert!(!serialized.contains("github-refresh-fail-refresh"));
    assert!(!serialized.contains("RAW_PROVIDER_ERROR_SENTINEL"));
    assert!(!serialized.contains("/host/path"));
}

#[tokio::test]
async fn concurrent_original_refresh_returns_pending_then_exact_published_result() {
    let services = Arc::new(InMemoryAuthProductServices::new());
    let provider_client = Arc::new(BlockingRefreshProvider::new(services.clone()));
    let auth = Arc::new(ProviderBackedCredentialAccountService::new(
        services.clone(),
        services.clone(),
        provider_client.clone(),
    ));

    let owner = scope("alice");
    let account = auth
        .create_account(NewCredentialAccount {
            scope: owner.clone(),
            provider: provider(),
            label: label("work"),
            status: CredentialAccountStatus::Expired,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: Vec::new(),
            access_secret: Some(SecretHandle::new("github-concurrent-access").unwrap()),
            access_expires_at: None,
            refresh_secret: Some(SecretHandle::new("github-concurrent-refresh").unwrap()),
            scopes: provider_scopes(&["repo"]),
        })
        .await
        .expect("expired account");

    let request = CredentialRefreshRequest::new(owner.clone(), provider(), account.id);
    let first_refresh = {
        let auth = auth.clone();
        let request = request.clone();
        tokio::spawn(async move { auth.refresh_account(request).await })
    };
    provider_client.wait_for_first_refresh_start().await;
    // Real original readback is immediate and does not wait behind a local
    // mutex or a timing/yield loop. The issuer is still explicitly blocked.
    let pending = auth.refresh_account(request.clone()).await.unwrap();
    assert!(!pending.refreshed);
    let receipt = pending.original.as_ref().unwrap();
    assert_eq!(receipt.id, request.id);
    assert_eq!(
        receipt.publication,
        brassclaw_auth::CredentialRefreshPublication::Pending
    );
    assert_eq!(
        receipt.outcome,
        brassclaw_auth::CredentialRefreshOutcomeKind::Unconfirmed
    );
    assert_eq!(pending.account.status, CredentialAccountStatus::Expired);
    assert_eq!(provider_client.refresh_call_count(), 1);
    provider_client.release_refresh();
    let published = first_refresh.await.unwrap().unwrap();
    assert!(published.refreshed);
    assert_eq!(auth.refresh_account(request).await.unwrap(), published);
    assert_eq!(provider_client.refresh_call_count(), 1);
    let stored = auth
        .get_account(CredentialAccountLookupRequest::new(owner, account.id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.status, CredentialAccountStatus::Configured);
}

#[tokio::test]
async fn stale_refresh_success_does_not_overwrite_concurrent_refresh() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let concurrent_access = SecretHandle::new("github-concurrent-access").unwrap();
    let concurrent_refresh = SecretHandle::new("github-concurrent-refresh").unwrap();
    let account = services
        .create_account(NewCredentialAccount {
            scope: owner.clone(),
            provider: provider(),
            label: label("work"),
            status: CredentialAccountStatus::Expired,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: Vec::new(),
            access_secret: Some(SecretHandle::new("github-stale-access").unwrap()),
            access_expires_at: None,
            refresh_secret: Some(SecretHandle::new("github-stale-refresh").unwrap()),
            scopes: provider_scopes(&["repo"]),
        })
        .await
        .expect("expired account");
    services.complete_refresh_during_next_provider_call_for_tests(
        account.id,
        concurrent_access.clone(),
        concurrent_refresh.clone(),
    );

    let report = services
        .refresh_account(CredentialRefreshRequest::new(
            owner.clone(),
            provider(),
            account.id,
        ))
        .await
        .expect("stale result remains accountable without overwriting newer credentials");

    assert!(!report.refreshed);
    assert_eq!(
        report.original.unwrap().publication,
        brassclaw_auth::CredentialRefreshPublication::Superseded
    );
    let stored = services
        .get_account(CredentialAccountLookupRequest::new(
            owner.clone(),
            account.id,
        ))
        .await
        .expect("lookup")
        .expect("account");
    assert_eq!(stored.status, CredentialAccountStatus::Configured);
    assert_eq!(stored.access_secret, Some(concurrent_access));
    assert_eq!(stored.refresh_secret, Some(concurrent_refresh));
}

#[tokio::test]
async fn stale_refresh_failure_does_not_mark_concurrent_refresh_failed() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let concurrent_access = SecretHandle::new("github-concurrent-failed-access").unwrap();
    let concurrent_refresh = SecretHandle::new("github-concurrent-failed-refresh").unwrap();
    let account = services
        .create_account(NewCredentialAccount {
            scope: owner.clone(),
            provider: provider(),
            label: label("work"),
            status: CredentialAccountStatus::Expired,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: Vec::new(),
            access_secret: Some(SecretHandle::new("github-old-failed-access").unwrap()),
            access_expires_at: None,
            refresh_secret: Some(SecretHandle::new("github-old-failed-refresh").unwrap()),
            scopes: provider_scopes(&["repo"]),
        })
        .await
        .expect("expired account");
    services.complete_refresh_during_next_provider_call_for_tests(
        account.id,
        concurrent_access.clone(),
        concurrent_refresh.clone(),
    );
    services.fail_next_refresh_for_tests(account.id);

    let report = services
        .refresh_account(CredentialRefreshRequest::new(
            owner.clone(),
            provider(),
            account.id,
        ))
        .await
        .expect("stale failure reports current account state");

    assert!(!report.refreshed);
    assert_eq!(report.account.status, CredentialAccountStatus::Configured);
    assert_eq!(report.recovery.kind(), CredentialRecoveryKind::Configured);
    let stored = services
        .get_account(CredentialAccountLookupRequest::new(
            owner.clone(),
            account.id,
        ))
        .await
        .expect("lookup")
        .expect("account");
    assert_eq!(stored.status, CredentialAccountStatus::Configured);
    assert_eq!(stored.access_secret, Some(concurrent_access));
    assert_eq!(stored.refresh_secret, Some(concurrent_refresh));
}

#[tokio::test]
async fn credential_refresh_without_refresh_secret_becomes_recoverable_status() {
    let services = Arc::new(InMemoryAuthProductServices::new());
    let auth = provider_backed_auth(services.clone());
    let owner = scope("alice");
    let account = auth
        .create_account(NewCredentialAccount {
            scope: owner.clone(),
            provider: provider(),
            label: label("work"),
            status: CredentialAccountStatus::Expired,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: Vec::new(),
            access_secret: Some(SecretHandle::new("github-refresh-no-refresh").unwrap()),
            access_expires_at: None,
            refresh_secret: None,
            scopes: provider_scopes(&["repo"]),
        })
        .await
        .expect("expired account");

    let report = auth
        .refresh_account(CredentialRefreshRequest::new(
            owner.clone(),
            provider(),
            account.id,
        ))
        .await
        .expect("missing refresh secret is projected");

    assert!(!report.refreshed);
    assert_eq!(
        report.account.status,
        CredentialAccountStatus::RefreshFailed
    );
    assert_eq!(
        report.recovery.kind(),
        CredentialRecoveryKind::ReauthorizeRequired
    );
    assert_eq!(
        report.recovery.reason,
        CredentialRecoveryReason::RefreshFailed
    );

    let failed = services
        .get_account(CredentialAccountLookupRequest::new(
            owner.clone(),
            account.id,
        ))
        .await
        .expect("lookup")
        .expect("failed account");
    assert_eq!(failed.status, CredentialAccountStatus::RefreshFailed);
}

#[tokio::test]
async fn provider_backed_refresh_preserves_requester_for_authorized_extensions() {
    let services = Arc::new(InMemoryAuthProductServices::new());
    let auth = provider_backed_auth(services.clone());
    let owner = scope("alice");
    let extension_owned = ExtensionId::new("github-extension-owned").unwrap();
    let shared_admin = ExtensionId::new("github-shared-admin").unwrap();

    let extension_account = auth
        .create_account(NewCredentialAccount {
            scope: owner.clone(),
            provider: provider(),
            label: label("extension owned"),
            status: CredentialAccountStatus::Expired,
            ownership: CredentialOwnership::ExtensionOwned,
            owner_extension: Some(extension_owned.clone()),
            granted_extensions: Vec::new(),
            access_secret: Some(SecretHandle::new("github-extension-owned-access").unwrap()),
            access_expires_at: None,
            refresh_secret: Some(SecretHandle::new("github-extension-owned-refresh").unwrap()),
            scopes: provider_scopes(&["repo"]),
        })
        .await
        .expect("extension-owned account");

    let extension_report = auth
        .refresh_account(
            CredentialRefreshRequest::new(owner.clone(), provider(), extension_account.id)
                .for_extension(extension_owned.clone()),
        )
        .await
        .expect("extension-owned refresh");
    assert!(extension_report.refreshed);
    assert_eq!(
        extension_report.account.status,
        CredentialAccountStatus::Configured
    );
    assert_eq!(
        extension_report.recovery.kind(),
        CredentialRecoveryKind::Configured
    );
    assert_eq!(
        extension_report
            .recovery
            .selected_account()
            .map(|account| account.id),
        Some(extension_account.id)
    );

    let shared_account = auth
        .create_account(NewCredentialAccount {
            scope: owner.clone(),
            provider: provider(),
            label: label("shared admin"),
            status: CredentialAccountStatus::Expired,
            ownership: CredentialOwnership::SharedAdminManaged,
            owner_extension: None,
            granted_extensions: vec![shared_admin.clone()],
            access_secret: Some(SecretHandle::new("github-shared-admin-access").unwrap()),
            access_expires_at: None,
            refresh_secret: Some(SecretHandle::new("github-shared-admin-refresh").unwrap()),
            scopes: provider_scopes(&["repo"]),
        })
        .await
        .expect("shared-admin account");

    let shared_report = auth
        .refresh_account(
            CredentialRefreshRequest::new(owner, provider(), shared_account.id)
                .for_extension(shared_admin),
        )
        .await
        .expect("shared-admin refresh");
    assert!(shared_report.refreshed);
    assert_eq!(
        shared_report.account.status,
        CredentialAccountStatus::Configured
    );
    assert_eq!(
        shared_report.recovery.kind(),
        CredentialRecoveryKind::Configured
    );
    assert_eq!(
        shared_report
            .recovery
            .selected_account()
            .map(|account| account.id),
        Some(shared_account.id)
    );
}

#[tokio::test]
async fn credential_refresh_rejects_terminal_statuses_even_with_refresh_secret() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");

    for (status, label_value, secret_suffix) in [
        (
            CredentialAccountStatus::Revoked,
            "terminal revoked",
            "revoked",
        ),
        (
            CredentialAccountStatus::Inactive,
            "terminal inactive",
            "inactive",
        ),
        (
            CredentialAccountStatus::PendingSetup,
            "terminal pending",
            "pending",
        ),
    ] {
        let account = services
            .create_account(NewCredentialAccount {
                scope: owner.clone(),
                provider: provider(),
                label: label(label_value),
                status,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(
                    SecretHandle::new(format!("github-terminal-access-{secret_suffix}")).unwrap(),
                ),
                access_expires_at: None,
                refresh_secret: Some(
                    SecretHandle::new(format!("github-terminal-refresh-{secret_suffix}")).unwrap(),
                ),
                scopes: provider_scopes(&["repo"]),
            })
            .await
            .expect("terminal account");

        let error = services
            .refresh_account(CredentialRefreshRequest::new(
                owner.clone(),
                provider(),
                account.id,
            ))
            .await
            .expect_err("terminal account refresh is rejected");
        assert_eq!(error, AuthProductError::CredentialMissing);

        let stored = services
            .get_account(CredentialAccountLookupRequest::new(
                owner.clone(),
                account.id,
            ))
            .await
            .expect("lookup")
            .expect("terminal account remains");
        assert_eq!(stored.status, status);
    }
}

#[tokio::test]
async fn credential_refresh_revalidates_scope_provider_and_grants() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let granted_extension = ExtensionId::new("github-extension").unwrap();
    let other_extension = ExtensionId::new("other-extension").unwrap();
    let account = services
        .create_account(NewCredentialAccount {
            scope: owner.clone(),
            provider: provider(),
            label: label("shared"),
            status: CredentialAccountStatus::Expired,
            ownership: CredentialOwnership::SharedAdminManaged,
            owner_extension: None,
            granted_extensions: vec![granted_extension.clone()],
            access_secret: Some(SecretHandle::new("github-shared-access").unwrap()),
            access_expires_at: None,
            refresh_secret: Some(SecretHandle::new("github-shared-refresh").unwrap()),
            scopes: provider_scopes(&["repo"]),
        })
        .await
        .expect("shared account");

    let no_requester = services
        .refresh_account(CredentialRefreshRequest::new(
            owner.clone(),
            provider(),
            account.id,
        ))
        .await
        .expect_err("shared account requires explicit grant");
    assert_eq!(no_requester, AuthProductError::CrossScopeDenied);

    let wrong_requester = services
        .refresh_account(
            CredentialRefreshRequest::new(owner.clone(), provider(), account.id)
                .for_extension(other_extension),
        )
        .await
        .expect_err("wrong extension cannot refresh shared account");
    assert_eq!(wrong_requester, AuthProductError::CrossScopeDenied);

    let provider_mismatch = services
        .refresh_account(
            CredentialRefreshRequest::new(
                owner.clone(),
                AuthProviderId::new("gitlab").unwrap(),
                account.id,
            )
            .for_extension(granted_extension.clone()),
        )
        .await
        .expect_err("provider mismatch rejected");
    assert_eq!(provider_mismatch, AuthProductError::CrossScopeDenied);

    let cross_scope = services
        .refresh_account(
            CredentialRefreshRequest::new(scope("bob"), provider(), account.id)
                .for_extension(granted_extension.clone()),
        )
        .await
        .expect_err("cross-scope refresh rejected");
    assert_eq!(cross_scope, AuthProductError::CrossScopeDenied);

    let report = services
        .refresh_account(
            CredentialRefreshRequest::new(owner, provider(), account.id)
                .for_extension(granted_extension),
        )
        .await
        .expect("granted extension can refresh");
    assert!(report.refreshed);
}

#[test]
fn provider_refresh_request_debug_redacts_secret_handle() {
    let request = OAuthProviderRefreshRequest {
        refresh_id: brassclaw_auth::CredentialRefreshId::new(),
        provider: provider(),
        scope: scope("alice"),
        account_id: brassclaw_auth::CredentialAccountId::new(),
        refresh_secret: SecretHandle::new("github-debug-refresh-secret").unwrap(),
        scopes: provider_scopes(&["repo"]),
    };
    let rendered = format!("{request:?}");
    assert!(rendered.contains("[REDACTED]"));
    assert!(!rendered.contains("github-debug-refresh-secret"));
    let access = SecretHandle::new("debug-access-sentinel").unwrap();
    let refresh = request.refresh_secret.clone();
    let destinations = brassclaw_auth::OAuthTokenDestinations {
        access_secret: access.clone(),
        refresh_secret: refresh.clone(),
    };
    let result = OAuthProviderRefresh {
        provider: provider(),
        access_secret: access.clone(),
        access_expires_at: None,
        refresh_secret: Some(refresh.clone()),
        scopes: vec![],
    };
    let exchange = OAuthProviderExchange {
        provider: provider(),
        account_label: label("debug metadata"),
        authorization_code_hash: code_hash("debug code"),
        pkce_verifier_hash: pkce_hash("debug pkce"),
        access_secret: access,
        access_expires_at: None,
        refresh_secret: Some(refresh),
        scopes: vec![],
        account_id: None,
    };
    for rendered in [
        format!("{destinations:?}"),
        format!("{result:?}"),
        format!("{exchange:?}"),
    ] {
        assert!(rendered.contains("[REDACTED]"));
        assert!(!rendered.contains("github-debug-refresh-secret"));
        assert!(!rendered.contains("debug-access-sentinel"));
    }
}

/// Mutate the real account exactly after the wrapper's last read, where a
/// read-then-unconditional-update implementation previously lost the edit.
struct RevokeBeforeRefreshWrite {
    inner: Arc<InMemoryAuthProductServices>,
    fenced_writes: AtomicUsize,
}
#[async_trait]
impl CredentialSetupService for RevokeBeforeRefreshWrite {
    async fn refresh_action(
        &self,
        request: &CredentialRefreshRequest,
    ) -> Result<Option<brassclaw_auth::CredentialRefreshAction>, AuthProductError> {
        self.inner.refresh_action(request).await
    }
    async fn begin_refresh(
        &self,
        prepared: brassclaw_auth::CredentialRefreshPrepared,
    ) -> Result<brassclaw_auth::CredentialRefreshAdmission, AuthProductError> {
        if prepared.expected_account.refresh_secret.is_none() {
            self.fenced_writes.fetch_add(1, Ordering::SeqCst);
            self.inner
                .update_status(
                    &prepared.original.scope,
                    prepared.original.account_id,
                    CredentialAccountStatus::Revoked,
                )
                .await?;
        }
        self.inner.begin_refresh(prepared).await
    }
    async fn reconcile_refresh(
        &self,
        request: &brassclaw_auth::CredentialRefreshRequest,
    ) -> Result<Option<brassclaw_auth::CredentialRefreshAction>, AuthProductError> {
        self.inner.reconcile_refresh(request).await
    }

    async fn complete_refresh(
        &self,
        completion: brassclaw_auth::CredentialRefreshCompletion,
    ) -> Result<brassclaw_auth::CredentialRefreshAction, AuthProductError> {
        self.fenced_writes.fetch_add(1, Ordering::SeqCst);
        let original = &completion.dispatched.original;
        self.inner
            .update_status(
                &original.scope,
                original.account_id,
                CredentialAccountStatus::Revoked,
            )
            .await?;
        self.inner.complete_refresh(completion).await
    }

    async fn compare_and_update_account(
        &self,
        expected: &CredentialAccount,
        request: NewCredentialAccount,
    ) -> Result<CredentialAccount, AuthProductError> {
        self.fenced_writes.fetch_add(1, Ordering::SeqCst);
        self.inner
            .update_status(
                &expected.scope,
                expected.id,
                CredentialAccountStatus::Revoked,
            )
            .await?;
        self.inner
            .compare_and_update_account(expected, request)
            .await
    }
    async fn create_or_update_account(
        &self,
        _request: CredentialAccountMutation,
    ) -> Result<CredentialAccount, AuthProductError> {
        panic!("refresh may not use an unconditional setup write")
    }
}

#[tokio::test]
async fn refresh_final_atomic_fence_preserves_revocation_for_every_write_outcome() {
    for outcome in ["success", "issuer-failure", "no-refresh-token"] {
        let services = Arc::new(InMemoryAuthProductServices::new());
        let owner = scope("alice");
        let original_access = SecretHandle::new("original-fenced-access").unwrap();
        let original_refresh = (outcome != "no-refresh-token")
            .then(|| SecretHandle::new("original-fenced-refresh").unwrap());
        let account = services
            .create_account(NewCredentialAccount {
                scope: owner.clone(),
                provider: provider(),
                label: label("fenced"),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: vec![],
                access_secret: Some(original_access.clone()),
                access_expires_at: None,
                refresh_secret: original_refresh.clone(),
                scopes: vec![],
            })
            .await
            .unwrap();
        if outcome == "issuer-failure" {
            services.fail_next_refresh_for_tests(account.id);
        }
        let setup = Arc::new(RevokeBeforeRefreshWrite {
            inner: services.clone(),
            fenced_writes: AtomicUsize::new(0),
        });
        let auth = ProviderBackedCredentialAccountService::new(
            services.clone(),
            setup.clone(),
            services.clone(),
        );
        let report = auth
            .refresh_account(CredentialRefreshRequest::new(
                owner.clone(),
                provider(),
                account.id,
            ))
            .await
            .unwrap();
        assert!(!report.refreshed, "{outcome}");
        assert_eq!(
            report.original.unwrap().publication,
            brassclaw_auth::CredentialRefreshPublication::Superseded,
            "{outcome}"
        );
        assert_eq!(setup.fenced_writes.load(Ordering::SeqCst), 1, "{outcome}");
        let retained = services
            .get_account(CredentialAccountLookupRequest::new(owner, account.id))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            retained.status,
            CredentialAccountStatus::Revoked,
            "{outcome}"
        );
        assert_eq!(retained.access_secret, Some(original_access), "{outcome}");
        assert_eq!(retained.refresh_secret, original_refresh, "{outcome}");
    }
}

#[tokio::test]
async fn cancelled_refresh_keeps_original_unconfirmed_and_blocks_replacement_dispatch() {
    let services = Arc::new(InMemoryAuthProductServices::new());
    let provider_client = Arc::new(BlockingRefreshProvider::new(services.clone()));
    let owner = scope("cancelled-owner");
    let mut create = account_request(
        owner.clone(),
        "cancelled-account",
        CredentialAccountStatus::Expired,
    );
    create.access_secret = Some(SecretHandle::new("cancelled-original-access").unwrap());
    create.refresh_secret = Some(SecretHandle::new("cancelled-original-refresh").unwrap());
    let account = services.create_account(create).await.unwrap();
    let request = CredentialRefreshRequest::new(owner.clone(), provider(), account.id);
    let wrapper = Arc::new(ProviderBackedCredentialAccountService::new(
        services.clone(),
        services.clone(),
        provider_client.clone(),
    ));
    let task = {
        let request = request.clone();
        let wrapper = wrapper.clone();
        tokio::spawn(async move { wrapper.refresh_account(request).await })
    };
    provider_client.wait_for_first_refresh_start().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    drop(wrapper);
    // A new coordinator has no old process-local mutex. The store owns replay fencing.
    let restarted = ProviderBackedCredentialAccountService::new(
        services.clone(),
        services.clone(),
        provider_client.clone(),
    );
    let report = restarted.refresh_account(request.clone()).await.unwrap();
    let receipt = report.original.unwrap();
    assert_eq!(receipt.id, request.id);
    assert_eq!(
        receipt.outcome,
        brassclaw_auth::CredentialRefreshOutcomeKind::Unconfirmed
    );
    assert_eq!(
        receipt.publication,
        brassclaw_auth::CredentialRefreshPublication::Pending
    );
    assert!(!report.refreshed);
    assert_eq!(provider_client.refresh_call_count(), 1);
    let replacement = CredentialRefreshRequest::new(owner.clone(), provider(), account.id);
    assert!(matches!(
        restarted.refresh_account(replacement).await,
        Err(AuthProductError::BackendConflict)
    ));
    assert_eq!(provider_client.refresh_call_count(), 1);
    let current = services
        .get_account(CredentialAccountLookupRequest::new(owner, account.id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current, account);
    let original = services.refresh_action(&request).await.unwrap().unwrap();
    assert!(matches!(
        original.outcome,
        brassclaw_auth::CredentialRefreshOutcome::Submitted
    ));
}

#[tokio::test]
async fn explicit_original_refresh_rejects_changed_body_and_keeps_exact_published_result() {
    let services = Arc::new(InMemoryAuthProductServices::new());
    let wrapper = provider_backed_auth(services.clone());
    let owner = scope("original-owner");
    let mut create = account_request(
        owner.clone(),
        "original-result",
        CredentialAccountStatus::Expired,
    );
    create.access_secret = Some(SecretHandle::new("original-result-access").unwrap());
    create.refresh_secret = Some(SecretHandle::new("original-result-refresh").unwrap());
    let account = services.create_account(create).await.unwrap();
    let request = CredentialRefreshRequest::new(owner.clone(), provider(), account.id);
    let result = wrapper.refresh_account(request.clone()).await.unwrap();
    assert!(result.refreshed);
    let restarted = provider_backed_auth(services.clone());
    assert_eq!(
        restarted.refresh_account(request.clone()).await.unwrap(),
        result
    );
    let mut changed = request.clone();
    changed.requester_extension = Some(ExtensionId::new("different-extension").unwrap());
    assert!(matches!(
        restarted.refresh_account(changed).await,
        Err(AuthProductError::BackendConflict)
    ));
    let other = services
        .create_account(account_request(
            owner.clone(),
            "other-account",
            CredentialAccountStatus::Expired,
        ))
        .await
        .unwrap();
    let changed =
        CredentialRefreshRequest::new(owner.clone(), provider(), other.id).with_id(request.id);
    assert!(matches!(
        restarted.refresh_account(changed).await,
        Err(AuthProductError::BackendConflict)
    ));
    let revoked = services
        .update_status(&owner, account.id, CredentialAccountStatus::Revoked)
        .await
        .unwrap();
    let stale = restarted.refresh_account(request.clone()).await.unwrap();
    assert!(!stale.refreshed);
    assert_eq!(stale.original, result.original);
    assert_eq!(stale.account, revoked.projection());
    let serialized = serde_json::to_string(&stale).unwrap();
    assert!(!serialized.contains("original-result-refresh"));
    assert!(!serialized.contains("original-result-access"));
}

#[tokio::test]
async fn legacy_refresh_intent_without_source_stays_readable_and_cannot_gain_dispatch() {
    use brassclaw_auth::{
        CredentialRefreshAction, CredentialRefreshAdmission, CredentialRefreshOriginal,
        CredentialRefreshPrepared, CredentialSetupService,
    };
    let services = InMemoryAuthProductServices::new();
    let owner = scope("legacy-refresh-generation");
    let account = services
        .create_account(NewCredentialAccount {
            scope: owner.clone(),
            provider: provider(),
            label: label("legacy generation"),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: None,
            access_expires_at: None,
            refresh_secret: Some(SecretHandle::new("legacy-refresh-source").unwrap()),
            scopes: vec![],
        })
        .await
        .unwrap();
    let request = CredentialRefreshRequest::new(owner, account.provider.clone(), account.id);
    let input = OAuthProviderRefreshRequest {
        refresh_id: request.id,
        provider: account.provider.clone(),
        scope: account.scope.clone(),
        account_id: account.id,
        refresh_secret: account.refresh_secret.clone().unwrap(),
        scopes: account.scopes.clone(),
    };
    let preparation = services.prepare_refresh(&input).await.unwrap();
    let CredentialRefreshAdmission::Dispatch(action) = services
        .begin_refresh(CredentialRefreshPrepared {
            original: CredentialRefreshOriginal::from(&request),
            expected_account: account,
            preparation: Some(preparation),
        })
        .await
        .unwrap()
    else {
        panic!("first test owner dispatch")
    };
    assert!(action.provider_dispatch().is_ok());
    let selected_reference = &action.source.as_ref().unwrap().record_reference;
    assert!(!format!("{action:?}").contains(selected_reference));
    assert!(
        !serde_json::to_string(&action.receipt().unwrap())
            .unwrap()
            .contains(selected_reference)
    );
    let mut json = serde_json::to_value(&action).unwrap();
    json.as_object_mut().unwrap().remove("source");
    let legacy: CredentialRefreshAction = serde_json::from_value(json.clone()).unwrap();
    assert!(legacy.source.is_none());
    assert_eq!(
        serde_json::to_value(&legacy).unwrap(),
        json,
        "no invented source backfill"
    );
    assert_eq!(legacy.receipt().unwrap(), action.receipt().unwrap());
    assert!(matches!(
        legacy.provider_dispatch(),
        Err(AuthProductError::BackendUnavailable)
    ));
    let mut replaced = action.clone();
    replaced.source.as_mut().unwrap().record_reference = "a".repeat(64);
    assert!(
        !action.same_intent(&replaced),
        "completion cannot substitute source generation"
    );
}

// Pure custody/state-transition contracts. No secret, real transport, native
// journal admission or installed-provider authority is manufactured here.
mod host_instance_state_contract {
    use super::*;
    use brassclaw_auth::{
        CredentialRefreshAction, CredentialRefreshAdmission, CredentialRefreshCompletion,
        CredentialRefreshId, CredentialRefreshOriginal, CredentialRefreshOutcome,
        CredentialRefreshPrepared, CredentialRefreshPublication, OAuthProviderRefreshPreparation,
        OAuthRefreshSource, OAuthTokenDestinations,
    };

    fn preparation() -> (CredentialAccount, CredentialRefreshPrepared) {
        // Native instance scope: SetupAdmin with no agent/project/thread/session.
        // local_default sets agent_id/project_id; clear them for a valid native subject.
        let mut resource = ResourceScope::local_default(
            UserId::new("original-installer").unwrap(),
            InvocationId::new(),
        )
        .unwrap();
        resource.agent_id = None;
        resource.project_id = None;
        let owner = AuthProductScope::new(resource, AuthSurface::SetupAdmin);
        let now = Utc::now();
        let account = CredentialAccount {
            id: CredentialAccountId::new(),
            scope: owner.clone(),
            provider: AuthProviderId::new("openai_codex").unwrap(),
            label: label("host instance metadata"),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::System,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: Some(SecretHandle::new("host-current-access").unwrap()),
            access_expires_at: None,
            refresh_secret: Some(SecretHandle::new("host-current-refresh").unwrap()),
            scopes: vec![],
            created_at: now,
            updated_at: now,
        };
        let request = CredentialRefreshRequest::new(owner, account.provider.clone(), account.id);
        let prepared = CredentialRefreshPrepared {
            original: CredentialRefreshOriginal::from(&request),
            expected_account: account.clone(),
            preparation: Some(OAuthProviderRefreshPreparation {
                destinations: OAuthTokenDestinations {
                    access_secret: SecretHandle::new("host-next-access").unwrap(),
                    refresh_secret: SecretHandle::new("host-next-refresh").unwrap(),
                },
                source: OAuthRefreshSource {
                    scope: account.scope.resource.clone(),
                    handle: account.refresh_secret.clone().unwrap(),
                    record_reference: "opaque fixture metadata, never authentication".into(),
                },
            }),
        };
        (account, prepared)
    }

    #[test]
    fn host_instance_metadata_never_enables_generic_system_refresh() {
        let (mut account, prepared) = preparation();
        assert!(matches!(
            CredentialRefreshAction::begin(&mut account, &[], prepared.clone(), Utc::now()),
            Err(AuthProductError::CrossScopeDenied)
        ));
        let CredentialRefreshAdmission::Dispatch(action) =
            CredentialRefreshAction::begin_host_instance(
                &mut account,
                &[],
                prepared.clone(),
                Utc::now(),
            )
            .unwrap()
        else {
            panic!("pure initial native metadata admission")
        };
        assert_eq!(account, prepared.expected_account);
        assert!(matches!(
            action.provider_dispatch(),
            Err(AuthProductError::CrossScopeDenied)
        ));
        let (request, selected) = action.provider_dispatch_host_instance().unwrap();
        assert_eq!(Some(&selected), prepared.preparation.as_ref());
        assert!(selected.source.verify(&request).is_ok());
        let restored: CredentialRefreshAction =
            serde_json::from_value(serde_json::to_value(&action).unwrap()).unwrap();
        assert_eq!(
            restored, action,
            "native ownership preserves ordinary stored scope identity"
        );
        assert!(matches!(
            restored.provider_dispatch(),
            Err(AuthProductError::CrossScopeDenied)
        ));
        let mut reserved = prepared.expected_account.clone();
        reserved.scope.resource = ResourceScope::system();
        let mut reserved_prepared = prepared.clone();
        reserved_prepared.expected_account = reserved.clone();
        reserved_prepared.original.scope = reserved.scope.clone();
        reserved_prepared.preparation.as_mut().unwrap().source.scope =
            reserved.scope.resource.clone();
        assert!(matches!(
            CredentialRefreshAction::begin_host_instance(
                &mut reserved,
                &[],
                reserved_prepared,
                Utc::now()
            ),
            Err(AuthProductError::CrossScopeDenied)
        ));
        assert!(
            serde_json::from_value::<OAuthRefreshSource>(
                serde_json::to_value(&OAuthRefreshSource {
                    scope: ResourceScope::system(),
                    ..selected.source.clone()
                })
                .unwrap()
            )
            .is_err()
        );
        let mut legacy = action.clone();
        legacy.source = None;
        assert!(matches!(
            legacy.provider_dispatch_host_instance(),
            Err(AuthProductError::BackendUnavailable)
        ));
    }

    #[test]
    fn host_instance_metadata_keeps_original_blockers_and_publication_fences() {
        let (mut account, prepared) = preparation();
        let CredentialRefreshAdmission::Dispatch(mut action) =
            CredentialRefreshAction::begin_host_instance(
                &mut account,
                &[],
                prepared.clone(),
                Utc::now(),
            )
            .unwrap()
        else {
            panic!("pure initial native metadata admission")
        };
        assert!(matches!(
            CredentialRefreshAction::begin_host_instance(
                &mut account,
                &[action.clone()],
                prepared.clone(),
                Utc::now()
            )
            .unwrap(),
            CredentialRefreshAdmission::Original(_)
        ));
        let mut alternate = prepared.clone();
        alternate.original.id = CredentialRefreshId::new();
        assert!(matches!(
            CredentialRefreshAction::begin_host_instance(
                &mut account,
                &[action.clone()],
                alternate,
                Utc::now()
            ),
            Err(AuthProductError::BackendConflict)
        ));
        let destinations = action.destinations.clone().unwrap();
        let completion = CredentialRefreshCompletion {
            dispatched: action.clone(),
            outcome: CredentialRefreshOutcome::Returned(OAuthProviderRefresh {
                provider: action.original.provider.clone(),
                access_secret: destinations.access_secret.clone(),
                access_expires_at: None,
                refresh_secret: Some(destinations.refresh_secret.clone()),
                scopes: vec![],
            }),
        };
        action.record_result(completion, Utc::now()).unwrap();
        action.publish(&mut account, Utc::now()).unwrap();
        assert_eq!(account.access_secret, Some(destinations.access_secret));
        assert_eq!(account.refresh_secret, Some(destinations.refresh_secret));
        assert_eq!(action.publication, CredentialRefreshPublication::Applied);
        assert!(action.receipt().unwrap().confirmed_refresh());
        assert!(matches!(
            CredentialRefreshAction::begin_host_instance(
                &mut account,
                &[action],
                prepared,
                Utc::now()
            )
            .unwrap(),
            CredentialRefreshAdmission::Original(_)
        ));
    }

    #[test]
    fn host_instance_metadata_rejects_scoped_extension_and_invalid_source_subjects() {
        let (account, prepared) = preparation();
        let mut extension = prepared.clone();
        extension.original.requester_extension =
            Some(ExtensionId::new("untrusted-extension").unwrap());
        assert!(matches!(
            CredentialRefreshAction::begin_host_instance(
                &mut account.clone(),
                &[],
                extension,
                Utc::now()
            ),
            Err(AuthProductError::CrossScopeDenied)
        ));
        for surface in [AuthSurface::Web, AuthSurface::Api, AuthSurface::Chat] {
            let mut wrong_surface = prepared.clone();
            wrong_surface.expected_account.scope.surface = surface;
            wrong_surface.original.scope.surface = surface;
            assert!(matches!(
                CredentialRefreshAction::begin_host_instance(
                    &mut account.clone(),
                    &[],
                    wrong_surface,
                    Utc::now()
                ),
                Err(AuthProductError::CrossScopeDenied)
            ));
        }
        let mut session = prepared.clone();
        session.expected_account.scope.session_id =
            Some(AuthSessionId::new("not-an-instance-original").unwrap());
        session.original.scope = session.expected_account.scope.clone();
        assert!(matches!(
            CredentialRefreshAction::begin_host_instance(
                &mut account.clone(),
                &[],
                session,
                Utc::now()
            ),
            Err(AuthProductError::CrossScopeDenied)
        ));
        let mut empty_source = prepared.clone();
        empty_source
            .preparation
            .as_mut()
            .unwrap()
            .source
            .record_reference
            .clear();
        assert!(matches!(
            CredentialRefreshAction::begin_host_instance(
                &mut account.clone(),
                &[],
                empty_source,
                Utc::now()
            ),
            Err(AuthProductError::BackendConflict)
        ));
        let mut wrong_source = prepared.clone();
        wrong_source.preparation.as_mut().unwrap().source.scope = ResourceScope::system();
        assert!(matches!(
            CredentialRefreshAction::begin_host_instance(
                &mut account.clone(),
                &[],
                wrong_source,
                Utc::now()
            ),
            Err(AuthProductError::CrossScopeDenied)
        ));
        let mut revoked = prepared.clone();
        revoked.expected_account.status = CredentialAccountStatus::Revoked;
        assert!(matches!(
            CredentialRefreshAction::begin_host_instance(
                &mut account.clone(),
                &[],
                revoked,
                Utc::now()
            ),
            Err(AuthProductError::CredentialMissing)
        ));
        let mut scoped = account;
        scoped.scope = scope("normal-account-remains-scoped");
        scoped.ownership = CredentialOwnership::UserReusable;
        let mut scoped_prepared = prepared;
        scoped_prepared.original.scope = scoped.scope.clone();
        scoped_prepared.expected_account = scoped.clone();
        scoped_prepared.preparation.as_mut().unwrap().source.scope = scoped.scope.resource.clone();
        assert!(matches!(
            CredentialRefreshAction::begin_host_instance(
                &mut scoped,
                &[],
                scoped_prepared.clone(),
                Utc::now()
            ),
            Err(AuthProductError::CrossScopeDenied)
        ));
        assert!(matches!(
            CredentialRefreshAction::begin(&mut scoped, &[], scoped_prepared, Utc::now()).unwrap(),
            CredentialRefreshAdmission::Dispatch(_)
        ));
    }
    #[test]
    fn host_confirmed_no_send_preserves_account_and_never_grants_redispatch() {
        let (mut account, prepared) = preparation();
        let before = account.clone();
        let CredentialRefreshAdmission::Dispatch(mut action) =
            CredentialRefreshAction::begin_host_instance(&mut account, &[], prepared, Utc::now())
                .unwrap()
        else {
            panic!("initial metadata admission")
        };
        let dispatched = action.clone();
        action
            .record_result(
                CredentialRefreshCompletion {
                    dispatched,
                    outcome: CredentialRefreshOutcome::NotSubmitted(
                        brassclaw_auth::CredentialRefreshStopReason::HostPreparationFailed,
                    ),
                },
                Utc::now(),
            )
            .unwrap();
        action.publish(&mut account, Utc::now()).unwrap();
        assert_eq!(account, before);
        assert!(!action.receipt().unwrap().confirmed_refresh());
        assert_eq!(
            action.receipt().unwrap().outcome,
            brassclaw_auth::CredentialRefreshOutcomeKind::NotSubmitted
        );
        assert!(action.provider_dispatch_host_instance().is_err());
    }
}
