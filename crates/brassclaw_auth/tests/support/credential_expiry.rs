//! Shared expiry projection contract; callers supply actual account owners.
use brassclaw_auth::*;

pub(super) async fn verify(accounts: &dyn CredentialAccountService, scope: AuthProductScope) {
    let provider = AuthProviderId::new("issuer-expiry-contract").unwrap();
    let deadline = chrono::Utc::now() + chrono::Duration::seconds(3);
    let raw = accounts
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: provider.clone(),
            label: CredentialAccountLabel::new("finite credential").unwrap(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: Some(
                brassclaw_host_api::SecretHandle::new("issuer-expiry-access").unwrap(),
            ),
            access_expires_at: Some(deadline),
            refresh_secret: None,
            scopes: vec![],
        })
        .await
        .unwrap();
    let snapshot = raw.snapshot_ref().unwrap();
    assert_eq!(
        accounts
            .select_configured_account(CredentialAccountChoiceRequest::new(
                scope.clone(),
                provider.clone(),
                raw.id
            ))
            .await
            .unwrap()
            .status,
        CredentialAccountStatus::Configured
    );
    let wait = (deadline - chrono::Utc::now()).to_std().unwrap_or_default();
    tokio::time::sleep(wait + std::time::Duration::from_millis(20)).await;
    let stored = accounts
        .get_account(CredentialAccountLookupRequest::new(scope.clone(), raw.id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        stored, raw,
        "elapsed time must not fabricate a raw concurrent edit"
    );
    assert_eq!(stored.snapshot_ref().unwrap(), snapshot);
    assert_eq!(stored.status, CredentialAccountStatus::Configured);
    assert_eq!(stored.projection().status, CredentialAccountStatus::Expired);
    assert_eq!(stored.projection().access_expires_at, Some(deadline));
    assert!(matches!(
        accounts
            .select_configured_account(CredentialAccountChoiceRequest::new(
                scope.clone(),
                provider.clone(),
                raw.id
            ))
            .await,
        Err(AuthProductError::CredentialMissing)
    ));
    assert!(matches!(
        accounts
            .select_unique_configured_account(CredentialAccountSelectionRequest::new(
                scope.clone(),
                provider.clone()
            ))
            .await,
        Err(AuthProductError::CredentialMissing)
    ));
    let recovery = accounts
        .project_credential_recovery(CredentialRecoveryRequest::new(
            scope.clone(),
            provider.clone(),
        ))
        .await
        .unwrap();
    assert_eq!(recovery.reason, CredentialRecoveryReason::AccountExpired);
    assert_eq!(recovery.kind(), CredentialRecoveryKind::ReauthorizeRequired);
    let list = accounts
        .list_accounts(CredentialAccountListRequest::new(scope, provider))
        .await
        .unwrap();
    assert_eq!(list.accounts.len(), 1);
    assert_eq!(list.accounts[0].status, CredentialAccountStatus::Expired);

    let mut legacy = raw;
    legacy.access_expires_at = None;
    let bytes = serde_json::to_vec(&legacy).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(value.get("access_expires_at").is_none());
    let restored: CredentialAccount = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored, legacy);
    assert_eq!(
        restored.snapshot_ref().unwrap(),
        legacy.snapshot_ref().unwrap()
    );
    assert_eq!(
        restored.effective_status(),
        CredentialAccountStatus::Configured
    );
}
