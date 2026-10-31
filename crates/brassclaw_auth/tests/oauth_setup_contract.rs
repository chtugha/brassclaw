//! Metadata contracts only; encrypted ownership/real stores require their own
//! caller acceptance when the setup path is wired.
use brassclaw_auth::{
    AuthContinuationRef, AuthFlowId, AuthFlowStatus, AuthProductError, AuthProductScope,
    AuthProviderId, AuthSurface, CredentialAccount, CredentialAccountId, CredentialAccountLabel,
    CredentialAccountStatus, CredentialAccountUpdateBinding, CredentialOwnership,
    OAuthAuthorizationUrl, OAuthSetupDefinition, OAuthSetupMaterialReference, OAuthSetupOriginal,
    OAuthSetupRetirement, OpaqueStateHash, PkceVerifierHash,
};
use brassclaw_host_api::{InvocationId, ResourceScope, SecretHandle, UserId};
use chrono::{Duration, Utc};

fn definition() -> OAuthSetupDefinition {
    let now = Utc::now();
    OAuthSetupDefinition {
        scope: AuthProductScope::new(
            ResourceScope::local_default(UserId::new("setup-owner").unwrap(), InvocationId::new())
                .unwrap(),
            AuthSurface::Web,
        ),
        flow_id: AuthFlowId::new(),
        provider: AuthProviderId::new("google").unwrap(),
        account_label: CredentialAccountLabel::new("original-account").unwrap(),
        requested_scopes: vec![],
        continuation: AuthContinuationRef::SetupOnly,
        requirement_setup: None,
        update_binding: None,
        update_account_snapshot: None,
        authorization_url: OAuthAuthorizationUrl::new("https://accounts.google.com/authorize")
            .unwrap(),
        opaque_state_hash: OpaqueStateHash::new("a".repeat(64)).unwrap(),
        pkce_verifier_hash: PkceVerifierHash::new("b".repeat(64)).unwrap(),
        created_at: now,
        expires_at: now + Duration::minutes(5),
    }
}

fn original() -> OAuthSetupOriginal {
    OAuthSetupOriginal {
        definition: definition(),
        preparation: OAuthSetupMaterialReference {
            handle: SecretHandle::new("google-setup-original").unwrap(),
            reference: "authenticated-first-generation".into(),
        },
    }
}

#[test]
fn original_generation_and_flow_identity_cannot_be_retargeted() {
    let first = original();
    let restored: OAuthSetupOriginal =
        serde_json::from_value(serde_json::to_value(&first).unwrap()).unwrap();
    first.verify(&restored).unwrap();
    let now = first.definition.created_at;
    let flow = first.definition.awaiting_flow(now).unwrap();
    first.definition.verify_flow(&flow).unwrap();
    for field in ["provider", "expiry", "generation", "target", "pkce"] {
        let mut changed = first.clone();
        match field {
            "provider" => changed.definition.provider = AuthProviderId::new("another").unwrap(),
            "expiry" => changed.definition.expires_at += Duration::seconds(1),
            "generation" => changed.preparation.reference = "replacement-generation".into(),
            "target" => {
                changed.definition.account_label =
                    CredentialAccountLabel::new("replacement").unwrap()
            }
            _ => {
                changed.definition.pkce_verifier_hash =
                    PkceVerifierHash::new("c".repeat(64)).unwrap()
            }
        }
        assert_eq!(
            first.verify(&changed).unwrap_err(),
            AuthProductError::BackendConflict
        );
    }
    let mut other = first.clone();
    other.definition.flow_id = AuthFlowId::new();
    assert_eq!(
        first.verify(&other).unwrap_err(),
        AuthProductError::CrossScopeDenied
    );
    let mut changed = flow;
    changed.created_at += Duration::seconds(1);
    assert_eq!(
        first.definition.verify_flow(&changed).unwrap_err(),
        AuthProductError::BackendConflict
    );
}

#[test]
fn bound_original_requires_the_original_account_snapshot_not_a_later_lookup() {
    let mut first = original();
    let now = first.definition.created_at;
    let account = CredentialAccount {
        id: CredentialAccountId::new(),
        scope: first.definition.scope.clone(),
        provider: first.definition.provider.clone(),
        label: first.definition.account_label.clone(),
        status: CredentialAccountStatus::Configured,
        ownership: CredentialOwnership::UserReusable,
        owner_extension: None,
        granted_extensions: vec![],
        access_secret: Some(SecretHandle::new("original-account-access").unwrap()),
        access_expires_at: None,
        refresh_secret: None,
        scopes: vec![],
        created_at: now,
        updated_at: now,
    };
    first.definition.update_binding = Some(CredentialAccountUpdateBinding {
        account_id: account.id,
        ownership: account.ownership,
        owner_extension: account.owner_extension.clone(),
        granted_extensions: account.granted_extensions.clone(),
    });
    assert_eq!(
        first.validate().unwrap_err(),
        AuthProductError::BackendConflict
    );
    first.definition.update_account_snapshot = Some(account);
    first.validate().unwrap();
    let mut changed = first.clone();
    changed
        .definition
        .update_account_snapshot
        .as_mut()
        .unwrap()
        .access_secret = Some(SecretHandle::new("a-later-generation").unwrap());
    changed.validate().unwrap();
    assert_eq!(
        first.verify(&changed).unwrap_err(),
        AuthProductError::BackendConflict
    );
    let mut wrong = first.clone();
    wrong
        .definition
        .update_account_snapshot
        .as_mut()
        .unwrap()
        .id = CredentialAccountId::new();
    assert_eq!(
        wrong.validate().unwrap_err(),
        AuthProductError::BackendConflict
    );
    let mut nil_target = first.clone();
    let nil = CredentialAccountId::from_uuid(uuid::Uuid::nil());
    nil_target
        .definition
        .update_binding
        .as_mut()
        .unwrap()
        .account_id = nil;
    nil_target
        .definition
        .update_account_snapshot
        .as_mut()
        .unwrap()
        .id = nil;
    assert_eq!(
        nil_target.validate().unwrap_err(),
        AuthProductError::BackendConflict
    );
    let terminal = first
        .definition
        .retired_flow(now, OAuthSetupRetirement::Canceled)
        .unwrap();
    assert_eq!(terminal.update_binding, first.definition.update_binding);
    first.definition.verify_flow(&terminal).unwrap();
}

#[test]
fn retirement_keeps_original_times_and_never_creates_callback_authority() {
    let first = original();
    let now = first.definition.created_at;
    assert!(
        first
            .definition
            .retired_flow(now, OAuthSetupRetirement::Expired)
            .is_err()
    );
    for (at, reason, status) in [
        (
            now,
            OAuthSetupRetirement::Canceled,
            AuthFlowStatus::Canceled,
        ),
        (
            first.definition.expires_at,
            OAuthSetupRetirement::Expired,
            AuthFlowStatus::Expired,
        ),
        (
            first.definition.expires_at,
            OAuthSetupRetirement::Canceled,
            AuthFlowStatus::Expired,
        ),
    ] {
        let mut retired = first.definition.retired_flow(at, reason).unwrap();
        assert_eq!(retired.status, status);
        assert_eq!(retired.created_at, first.definition.created_at);
        assert_eq!(retired.expires_at, first.definition.expires_at);
        assert!(retired.challenge.is_none());
        assert!(retired.credential_account_id.is_none());
        assert!(retired.authorization_code_hash.is_none());
        assert!(retired.continuation_emitted_at.is_none());
        first.definition.verify_flow(&retired).unwrap();
        assert!(
            brassclaw_auth::domain::prepare_callback_flow(
                &mut retired,
                &first.definition.scope,
                &first.definition.opaque_state_hash,
                at
            )
            .is_err()
        );
    }
    assert_eq!(
        first
            .definition
            .awaiting_flow(first.definition.expires_at)
            .unwrap_err(),
        AuthProductError::UnknownOrExpiredFlow
    );
}

#[test]
fn private_values_are_not_schema_fields_and_context_has_no_reference_cycle() {
    let first = original();
    for field in [
        "pkce_verifier",
        "client_secret",
        "authorization_code",
        "access_token",
    ] {
        let mut input = serde_json::to_value(&first.definition).unwrap();
        input[field] = serde_json::json!("raw-must-not-enter-Auth-metadata");
        assert!(serde_json::from_value::<OAuthSetupDefinition>(input).is_err());
    }
    let context = first.definition.preparation_context();
    assert_eq!(context["format"], "oauth-setup-original/1");
    assert!(!context.to_string().contains(&first.preparation.reference));
    let mut invalid = first;
    invalid.preparation.reference.clear();
    assert_eq!(
        invalid.validate().unwrap_err(),
        AuthProductError::BackendConflict
    );
    invalid.preparation.reference = "original".into();
    invalid.definition.expires_at = invalid.definition.created_at;
    assert_eq!(
        invalid.validate().unwrap_err(),
        AuthProductError::BackendConflict
    );
}

#[test]
fn custody_preserves_old_absence_and_roundtrips_the_complete_original() {
    use brassclaw_auth::AuthFlowCustody;
    assert_eq!(
        serde_json::to_value(AuthFlowCustody::default()).unwrap(),
        serde_json::json!({})
    );
    let first = original();
    let (flow, custody) =
        AuthFlowCustody::prepared_oauth_setup(first.clone(), None, first.definition.created_at)
            .unwrap();
    assert!(!custody.is_empty());
    let recovered: AuthFlowCustody =
        serde_json::from_value(serde_json::to_value(&custody).unwrap()).unwrap();
    let selected = recovered.oauth_setup_state(&flow).unwrap().unwrap();
    assert_eq!(selected.original, first);
    assert_eq!(selected.flow, flow);
    let legacy: AuthFlowCustody = serde_json::from_value(serde_json::json!({})).unwrap();
    assert!(legacy.oauth_setup_state(&flow).unwrap().is_none());
    let mut alias = first;
    alias.preparation.handle = alias.definition.exchange_use_handle().unwrap();
    assert_eq!(
        alias.validate().unwrap_err(),
        AuthProductError::BackendConflict
    );
}

#[tokio::test]
async fn existing_original_cancellation_and_recovery_never_rearm_the_flow() {
    use brassclaw_auth::{AuthFlowManager, InMemoryAuthProductServices};
    let auth = InMemoryAuthProductServices::new();
    let first = original();
    let stored = auth.prepare_oauth_setup(first.clone(), None).await.unwrap();
    assert_eq!(stored.original, first);
    assert_eq!(stored.flow.created_at, first.definition.created_at);
    assert_eq!(
        auth.prepare_oauth_setup(first.clone(), Some(OAuthSetupRetirement::Canceled))
            .await
            .unwrap(),
        stored,
        "existing-original readback does not claim that cancellation was performed"
    );
    let canceled = auth
        .cancel_flow(&first.definition.scope, first.definition.flow_id)
        .await
        .unwrap();
    assert_eq!(canceled.status, AuthFlowStatus::Canceled);
    let replay = auth.prepare_oauth_setup(first.clone(), None).await.unwrap();
    assert_eq!(replay.flow, canceled);
    assert_eq!(replay.original, first);
    let mut changed = first.clone();
    changed.preparation.reference = "a-new-generation".into();
    assert_eq!(
        auth.prepare_oauth_setup(changed, None).await.unwrap_err(),
        AuthProductError::BackendConflict
    );
    assert_eq!(
        auth.read_oauth_setup(&first.definition.scope, first.definition.flow_id)
            .await
            .unwrap()
            .unwrap(),
        replay
    );
    let mut expired = original();
    expired.definition.created_at = Utc::now() - Duration::minutes(2);
    expired.definition.expires_at = Utc::now() - Duration::minutes(1);
    let retired = auth
        .prepare_oauth_setup(expired.clone(), None)
        .await
        .unwrap();
    assert_eq!(retired.flow.status, AuthFlowStatus::Expired);
    assert_eq!(retired.original, expired);
    assert!(retired.flow.challenge.is_none());
}

#[test]
fn first_callback_must_match_setup_metadata_before_a_claim_or_mutation() {
    use brassclaw_auth::{
        AuthFlowCustody, AuthorizationCodeHash, OAuthCallbackAdmission, OAuthCallbackOriginal,
        OAuthCallbackPrepared, OAuthTokenDestinations, ProviderScope,
    };
    let first = original();
    let now = first.definition.created_at;
    let (flow, custody) = AuthFlowCustody::prepared_oauth_setup(first.clone(), None, now).unwrap();
    let good = OAuthCallbackPrepared {
        original: OAuthCallbackOriginal {
            scope: first.definition.scope.clone(),
            flow_id: first.definition.flow_id,
            provider: first.definition.provider.clone(),
            opaque_state_hash: first.definition.opaque_state_hash.clone(),
            authorization_code_hash: AuthorizationCodeHash::new("d".repeat(64)).unwrap(),
            pkce_verifier_hash: first.definition.pkce_verifier_hash.clone(),
            account_label: first.definition.account_label.clone(),
            requested_scopes: first.definition.requested_scopes.clone(),
        },
        destinations: OAuthTokenDestinations {
            access_secret: SecretHandle::new("first-callback-access").unwrap(),
            refresh_secret: SecretHandle::new("first-callback-refresh").unwrap(),
        },
    };
    for field in ["label", "scopes", "provider"] {
        let mut attempted = good.clone();
        match field {
            "label" => {
                attempted.original.account_label =
                    CredentialAccountLabel::new("another-label").unwrap()
            }
            "scopes" => {
                attempted.original.requested_scopes =
                    vec![ProviderScope::new("another-scope").unwrap()]
            }
            _ => attempted.original.provider = AuthProviderId::new("another-provider").unwrap(),
        }
        let mut untouched_flow = flow.clone();
        let mut untouched_custody = custody.clone();
        assert_eq!(
            untouched_custody
                .admit(&mut untouched_flow, None, attempted, now)
                .unwrap_err(),
            AuthProductError::BackendConflict
        );
        assert_eq!(untouched_flow, flow);
        assert_eq!(untouched_custody, custody);
    }
    let mut admitted_flow = flow;
    let mut admitted_custody = custody;
    assert!(matches!(
        admitted_custody
            .admit(&mut admitted_flow, None, good.clone(), now)
            .unwrap(),
        OAuthCallbackAdmission::Dispatch(_)
    ));
    assert!(matches!(
        admitted_custody
            .admit(&mut admitted_flow, None, good, now)
            .unwrap(),
        OAuthCallbackAdmission::Original(_)
    ));
}
