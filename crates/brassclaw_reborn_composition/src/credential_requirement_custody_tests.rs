//! Shared original-setup checks run through the actual native and FS owners.
//! This does not establish selection publication or task-family grants.
use brassclaw_auth::{
    AuthFlowManager, AuthProductError, AuthProductScope, CredentialRequirementSetup,
    ManualTokenSetupIdentity, ManualTokenSetupRequest,
};
use brassclaw_host_api::{
    ExtensionId, NetworkTargetPattern, RuntimeCredentialAccountProviderId,
    RuntimeCredentialAccountRequirement, RuntimeCredentialAccountSetup,
    RuntimeCredentialRequirement, RuntimeCredentialRequirementSource, RuntimeCredentialTarget,
    SecretHandle,
};
use chrono::{Duration, Utc};

use crate::manual_token_flow::RebornManualTokenFlowService;

#[tokio::test]
async fn in_memory_original_requirement_custody_rejects_reinterpretation() {
    let services = brassclaw_auth::InMemoryAuthProductServices::new();
    let scope = AuthProductScope::new(
        brassclaw_host_api::ResourceScope::local_default(
            brassclaw_host_api::UserId::new("requirement-owner").unwrap(),
            brassclaw_host_api::InvocationId::new(),
        )
        .unwrap(),
        brassclaw_auth::AuthSurface::Web,
    );
    verify(&services, &services, scope).await;
}

#[test]
fn absent_requirement_custody_preserves_historical_serialized_shape() {
    let old = serde_json::json!({});
    let custody: brassclaw_auth::AuthFlowCustody = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(serde_json::to_value(custody).unwrap(), old);
}

fn usage(provider: &str, audience: &str) -> RuntimeCredentialAccountRequirement {
    RuntimeCredentialAccountRequirement::new(
        ExtensionId::new("requirement-custody-extension").unwrap(),
        RuntimeCredentialRequirement {
            handle: SecretHandle::new("api-token").unwrap(),
            source: RuntimeCredentialRequirementSource::ProductAuthAccount {
                provider: RuntimeCredentialAccountProviderId::new(provider).unwrap(),
                setup: RuntimeCredentialAccountSetup::ManualToken,
            },
            provider_scopes: Vec::new(),
            audience: NetworkTargetPattern {
                scheme: None,
                host_pattern: audience.into(),
                port: None,
            },
            target: RuntimeCredentialTarget::Header {
                name: "Authorization".into(),
                prefix: Some("Bearer ".into()),
            },
            required: true,
        },
    )
    .unwrap()
}

pub(crate) async fn verify<T>(first: &T, reopened: &T, scope: AuthProductScope)
where
    T: RebornManualTokenFlowService + AuthFlowManager,
{
    let setup = CredentialRequirementSetup::new(
        &scope,
        usage("requirement-provider", "first.example.com"),
        None,
    )
    .unwrap();
    let request = ManualTokenSetupRequest {
        requirement_setup: Some(Box::new(setup.clone())),
        scope: scope.clone(),
        provider: brassclaw_auth::AuthProviderId::new("requirement-provider").unwrap(),
        label: brassclaw_auth::CredentialAccountLabel::new("original requirement").unwrap(),
        continuation: brassclaw_auth::AuthContinuationRef::SetupOnly,
        update_binding: None,
        expires_at: Utc::now() + Duration::minutes(5),
    };
    let identity = ManualTokenSetupIdentity::default();
    let initial = first
        .request_original_manual_token_flow(identity, request.clone())
        .await
        .unwrap();
    assert_eq!(
        reopened
            .requirement_setup(&scope, initial.id)
            .await
            .unwrap(),
        Some(setup.clone())
    );
    assert_eq!(
        reopened
            .recover_original_manual_token_flow(&scope, identity)
            .await
            .unwrap(),
        Some(initial.clone())
    );
    let mut retry = request.clone();
    retry.expires_at += Duration::hours(1);
    assert_eq!(
        reopened
            .request_original_manual_token_flow(identity, retry.clone())
            .await
            .unwrap(),
        initial
    );
    retry.requirement_setup = Some(Box::new(
        CredentialRequirementSetup::new(
            &scope,
            usage("requirement-provider", "different.example.com"),
            None,
        )
        .unwrap(),
    ));
    assert_eq!(
        reopened
            .request_original_manual_token_flow(identity, retry.clone())
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    retry.requirement_setup = None;
    assert_eq!(
        reopened
            .request_original_manual_token_flow(identity, retry)
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    assert_eq!(
        reopened
            .requirement_setup(&scope, initial.id)
            .await
            .unwrap(),
        Some(setup)
    );

    // A provider mismatch is rejected before either original record exists.
    let invalid_id = ManualTokenSetupIdentity::default();
    let mut invalid = request;
    invalid.provider = brassclaw_auth::AuthProviderId::new("another-provider").unwrap();
    assert_eq!(
        first
            .request_original_manual_token_flow(invalid_id, invalid)
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    assert!(
        reopened
            .recover_original_manual_token_flow(&scope, invalid_id)
            .await
            .unwrap()
            .is_none()
    );

    // Lower flow creation also retains setup in its first durable write.
    let flow = reopened
        .create_flow(brassclaw_auth::NewAuthFlow {
            requirement_setup: Some(Box::new(
                CredentialRequirementSetup::new(
                    &scope,
                    usage("requirement-provider", "selection.example.com"),
                    None,
                )
                .unwrap(),
            )),
            id: None,
            scope: scope.clone(),
            kind: brassclaw_auth::AuthFlowKind::IntegrationCredential,
            provider: brassclaw_auth::AuthProviderId::new("requirement-provider").unwrap(),
            challenge: brassclaw_auth::AuthChallenge::AccountSelectionRequired {
                provider: brassclaw_auth::AuthProviderId::new("requirement-provider").unwrap(),
                accounts: Vec::new(),
            },
            continuation: brassclaw_auth::AuthContinuationRef::SetupOnly,
            update_binding: None,
            opaque_state_hash: None,
            pkce_verifier_hash: None,
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();
    assert_eq!(
        first
            .requirement_setup(&scope, flow.id)
            .await
            .unwrap()
            .unwrap()
            .requirement(),
        &usage("requirement-provider", "selection.example.com")
    );
}

/// Actual original-flow owners; real secret stores additionally prove that
/// changed heads/material cannot replace the original encrypted generation.
/// No task-family grant or native Auth-wait producer is established here.
pub(crate) async fn verify_original_flow_selection<T>(
    services: &T,
    reopened: &T,
    scope: AuthProductScope,
    requirement: RuntimeCredentialAccountRequirement,
    secrets: Option<&dyn brassclaw_secrets::SecretStore>,
) where
    T: brassclaw_auth::CredentialAccountService
        + brassclaw_auth::CredentialAccountRecordSource
        + AuthFlowManager,
{
    use brassclaw_auth::{
        AuthChallenge, AuthContinuationRef, AuthFlowKind, AuthFlowStatus, CredentialAccountLabel,
        CredentialAccountStatus, CredentialOwnership, CredentialSelectionInput, NewAuthFlow,
        NewCredentialAccount, ProviderScope,
    };

    let RuntimeCredentialRequirementSource::ProductAuthAccount { provider, .. } =
        &requirement.requirement().source
    else {
        panic!("account usage fixture");
    };
    let provider = brassclaw_auth::AuthProviderId::new(provider.as_str()).unwrap();
    let account_request = |label: &str| NewCredentialAccount {
        scope: scope.clone(),
        provider: provider.clone(),
        label: CredentialAccountLabel::new(label).unwrap(),
        status: CredentialAccountStatus::Configured,
        ownership: CredentialOwnership::UserReusable,
        owner_extension: None,
        granted_extensions: Vec::new(),
        access_secret: Some(SecretHandle::new(label).unwrap()),
        access_expires_at: None,
        refresh_secret: None,
        scopes: requirement
            .requirement()
            .provider_scopes
            .iter()
            .map(|scope| ProviderScope::new(scope.clone()).unwrap())
            .collect(),
    };
    let mut accounts = Vec::new();
    for label in ["flow-original-a", "flow-original-b"] {
        // The Auth fake has metadata only. The real owners authenticate actual
        // encrypted material without issuing a runtime lease during selection.
        if let Some(secrets) = secrets {
            secrets
                .put(
                    scope.resource.clone(),
                    SecretHandle::new(label).unwrap(),
                    "original-flow test material".into(),
                )
                .await
                .unwrap();
        }
        accounts.push(
            services
                .create_account(account_request(label))
                .await
                .unwrap(),
        );
    }
    let request = |previous| NewAuthFlow {
        requirement_setup: Some(Box::new(
            CredentialRequirementSetup::new(&scope, requirement.clone(), previous).unwrap(),
        )),
        id: None,
        scope: scope.clone(),
        kind: AuthFlowKind::IntegrationCredential,
        provider: provider.clone(),
        challenge: AuthChallenge::AccountSelectionRequired {
            provider: provider.clone(),
            accounts: accounts
                .iter()
                .map(|account| account.projection())
                .collect(),
        },
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        opaque_state_hash: None,
        pkce_verifier_hash: None,
        expires_at: Utc::now() + Duration::minutes(5),
    };
    let first = services.create_flow(request(None)).await.unwrap();
    assert!(
        reopened
            .requirement_selection_for_flow(&scope, first.id, &requirement)
            .await
            .unwrap()
            .is_none()
    );
    services
        .complete_credential_selection(
            &scope,
            CredentialSelectionInput {
                flow_id: first.id,
                credential_account_id: accounts[0].id,
            },
        )
        .await
        .unwrap();
    let original = reopened
        .requirement_selection(&scope, &requirement)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original.account().id, accounts[0].id);
    assert_eq!(
        reopened
            .requirement_selection_for_flow(&scope, first.id, &requirement)
            .await
            .unwrap(),
        Some(original.clone())
    );

    let original_source = if let Some(secrets) = secrets {
        let source = reopened
            .requirement_source_for_flow(&scope, first.id, &requirement)
            .await
            .unwrap()
            .expect("actual original generation captured at choice");
        source.verify_selection(&original).unwrap();
        let handle = original.account().access_secret.as_ref().unwrap();
        assert_eq!(
            source.generation().as_str(),
            secrets
                .record_reference(&original.account().scope.resource, handle)
                .await
                .unwrap()
                .unwrap()
        );
        assert!(!format!("{source:?}").contains(source.generation().as_str()));
        let encoded = serde_json::to_value(&source).unwrap();
        assert_eq!(
            serde_json::from_value::<brassclaw_auth::CredentialRequirementSource>(encoded.clone())
                .unwrap(),
            source
        );
        for (field, value) in [
            (
                "format",
                serde_json::json!("credential-requirement-source/0"),
            ),
            ("generation", serde_json::json!(" invalid generation ")),
            ("unexpected", serde_json::json!(true)),
        ] {
            let mut invalid = encoded.clone();
            invalid[field] = value;
            assert!(
                serde_json::from_value::<brassclaw_auth::CredentialRequirementSource>(invalid)
                    .is_err()
            );
        }
        // Replacing ordinary material under the exact original handle does not
        // rewrite the flow's original proof or authorize leasing the replacement.
        secrets
            .put(
                original.account().scope.resource.clone(),
                handle.clone(),
                "replacement must never be adopted".into(),
            )
            .await
            .unwrap();
        let current = secrets
            .record_reference(&original.account().scope.resource, handle)
            .await
            .unwrap()
            .unwrap();
        assert_ne!(source.generation().as_str(), current);
        assert!(
            secrets
                .lease_once_if_record_reference(
                    &original.account().scope.resource,
                    handle,
                    source.generation().as_str()
                )
                .await
                .unwrap()
                .is_none()
        );
        Some(source)
    } else {
        assert_eq!(
            reopened
                .requirement_source_for_flow(&scope, first.id, &requirement)
                .await
                .unwrap_err(),
            AuthProductError::BackendUnavailable
        );
        None
    };

    let second = services
        .create_flow(request(Some(original.clone())))
        .await
        .unwrap();
    services
        .complete_credential_selection(
            &scope,
            CredentialSelectionInput {
                flow_id: second.id,
                credential_account_id: accounts[1].id,
            },
        )
        .await
        .unwrap();
    let replacement = reopened
        .requirement_selection(&scope, &requirement)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(replacement.account().id, accounts[1].id);
    if let Some(source) = &original_source {
        assert!(source.verify_selection(&replacement).is_err());
        assert_eq!(
            reopened
                .requirement_source_for_flow(&scope, first.id, &requirement)
                .await
                .unwrap()
                .as_ref(),
            Some(source)
        );
        assert!(
            reopened
                .requirement_source_for_flow(&scope, second.id, &requirement)
                .await
                .unwrap()
                .is_some()
        );
    }
    assert_ne!(
        replacement.reference().unwrap(),
        original.reference().unwrap()
    );
    // Read the original even though the mutable head now refers to B.
    assert_eq!(
        reopened
            .requirement_selection_for_flow(&scope, first.id, &requirement)
            .await
            .unwrap(),
        Some(original.clone())
    );
    assert_eq!(
        reopened
            .requirement_selection_for_flow(&scope, second.id, &requirement)
            .await
            .unwrap(),
        Some(replacement.clone())
    );

    let mut declaration = requirement.requirement().clone();
    declaration.audience.host_pattern = "different-original-flow.example".into();
    let different = RuntimeCredentialAccountRequirement::new(
        requirement.requester_extension().clone(),
        declaration,
    )
    .unwrap();
    assert!(
        reopened
            .requirement_selection_for_flow(&scope, first.id, &different)
            .await
            .is_err()
    );
    let mut foreign = scope.clone();
    foreign.resource.user_id =
        brassclaw_host_api::UserId::new("foreign-original-flow-owner").unwrap();
    assert!(!matches!(
        reopened
            .requirement_selection_for_flow(&foreign, first.id, &requirement)
            .await,
        Ok(Some(_))
    ));
    assert!(
        reopened
            .requirement_selection_for_flow(&scope, brassclaw_auth::AuthFlowId::new(), &requirement)
            .await
            .unwrap()
            .is_none()
    );

    let pending = services
        .create_flow(request(Some(replacement.clone())))
        .await
        .unwrap();
    assert_eq!(
        services
            .cancel_flow(&scope, pending.id)
            .await
            .unwrap()
            .status,
        AuthFlowStatus::Canceled
    );
    assert!(
        reopened
            .requirement_selection_for_flow(&scope, pending.id, &requirement)
            .await
            .unwrap()
            .is_none()
    );
    if secrets.is_some() {
        assert!(
            reopened
                .requirement_source_for_flow(&scope, pending.id, &requirement)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            reopened
                .requirement_source_for_flow(&scope, first.id, &different)
                .await
                .is_err()
        );
        assert!(!matches!(
            reopened
                .requirement_source_for_flow(&foreign, first.id, &requirement)
                .await,
            Ok(Some(_))
        ));
    }
    // A completed historical flow without exact setup cannot acquire today's head.
    let mut historical = request(None);
    historical.requirement_setup = None;
    let historical = services.create_flow(historical).await.unwrap();
    services
        .complete_credential_selection(
            &scope,
            CredentialSelectionInput {
                flow_id: historical.id,
                credential_account_id: accounts[0].id,
            },
        )
        .await
        .unwrap();
    assert!(
        reopened
            .requirement_selection_for_flow(&scope, historical.id, &requirement)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        reopened
            .requirement_selection(&scope, &requirement)
            .await
            .unwrap(),
        Some(replacement.clone())
    );

    if secrets.is_some() {
        assert!(
            reopened
                .requirement_source_for_flow(&scope, historical.id, &requirement)
                .await
                .unwrap()
                .is_none()
        );
    }
    // Helper inputs alone cannot promote malformed or incomplete custody.
    let completed = reopened.get_flow(&scope, second.id).await.unwrap().unwrap();
    let setup =
        CredentialRequirementSetup::new(&scope, requirement.clone(), Some(original)).unwrap();
    let valid = brassclaw_auth::AuthFlowCustody {
        requirement_setup: Some(Box::new(setup)),
        requirement_selection: Some(Box::new(replacement)),
        ..Default::default()
    };
    assert!(
        valid
            .original_requirement_selection(&scope, first.id, &completed, &requirement)
            .is_err()
    );
    for missing_setup in [false, true] {
        let mut malformed = valid.clone();
        if missing_setup {
            malformed.requirement_setup = None;
        } else {
            malformed.requirement_selection = None;
        }
        assert!(
            malformed
                .original_requirement_selection(&scope, second.id, &completed, &requirement)
                .is_err()
        );
    }
    if let Some(secrets) = secrets {
        // A distinct account starts with no material. No delete permission or
        // cleanup side effect is needed to establish genuine original absence.
        let account = services
            .create_account(account_request("flow-original-missing"))
            .await
            .unwrap();
        let handle = account.access_secret.as_ref().unwrap();
        let mut missing_request = request(Some(
            valid.requirement_selection.as_deref().unwrap().clone(),
        ));
        missing_request.challenge = AuthChallenge::AccountSelectionRequired {
            provider: provider.clone(),
            accounts: vec![account.projection()],
        };
        let missing = services.create_flow(missing_request).await.unwrap();
        services
            .complete_credential_selection(
                &scope,
                CredentialSelectionInput {
                    flow_id: missing.id,
                    credential_account_id: account.id,
                },
            )
            .await
            .unwrap();
        assert!(
            reopened
                .requirement_source_for_flow(&scope, missing.id, &requirement)
                .await
                .unwrap()
                .is_none()
        );
        secrets
            .put(
                account.scope.resource.clone(),
                handle.clone(),
                "later configuration cannot backfill an original".into(),
            )
            .await
            .unwrap();
        assert!(
            reopened
                .requirement_source_for_flow(&scope, missing.id, &requirement)
                .await
                .unwrap()
                .is_none()
        );
        if let Some(source) = original_source {
            let mut corrupt = valid.clone();
            corrupt.requirement_source = Some(Box::new(source));
            assert!(
                corrupt
                    .original_requirement_source(&scope, second.id, &completed, &requirement)
                    .is_err()
            );
        }
    }
}

#[tokio::test]
async fn in_memory_original_flow_selection_survives_changed_head_without_latest_fallback() {
    let services = brassclaw_auth::InMemoryAuthProductServices::new();
    let scope = AuthProductScope::new(
        brassclaw_host_api::ResourceScope::local_default(
            brassclaw_host_api::UserId::new("original-flow-selection-owner").unwrap(),
            brassclaw_host_api::InvocationId::new(),
        )
        .unwrap(),
        brassclaw_auth::AuthSurface::Web,
    );
    verify_original_flow_selection(
        &services,
        &services,
        scope,
        usage("requirement-provider", "original-flow.example"),
        None,
    )
    .await;
}

/// Confirm the actual original flow has a proof of its real encrypted output.
pub(crate) async fn published_source<T: brassclaw_auth::CredentialAccountRecordSource>(
    owner: &T,
    secrets: &dyn brassclaw_secrets::SecretStore,
    scope: &AuthProductScope,
    selected: &brassclaw_auth::CredentialRequirementSelection,
) -> brassclaw_auth::CredentialRequirementSource {
    let source = owner
        .requirement_source_for_flow(scope, selected.original_flow(), selected.requirement())
        .await
        .unwrap()
        .expect("real publication generation");
    source.verify_selection(selected).unwrap();
    assert_eq!(
        source.generation().as_str(),
        secrets
            .record_reference(
                &selected.account().scope.resource,
                selected.account().access_secret.as_ref().unwrap()
            )
            .await
            .unwrap()
            .unwrap()
    );
    source
}
