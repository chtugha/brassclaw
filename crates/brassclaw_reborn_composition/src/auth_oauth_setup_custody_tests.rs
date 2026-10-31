//! Actual encrypted/native owners; this does not execute a Google provider or
//! qualify the still-unconnected private client/PKCE codec.
use brassclaw_auth::{
    AuthContinuationRef, AuthFlowId, AuthFlowManager, AuthFlowStatus, AuthProductError,
    AuthProductScope, AuthProviderId, AuthorizationCodeHash, CredentialAccountLabel,
    CredentialAccountService, CredentialAccountStatus, CredentialAccountUpdateBinding,
    CredentialOwnership, NewAuthFlow, NewCredentialAccount, OAuthAuthorizationUrl,
    OAuthCallbackAdmission, OAuthCallbackOriginal, OAuthCallbackPrepared, OAuthSetupDefinition,
    OAuthSetupMaterialReference, OAuthSetupOriginal, OAuthSetupRetirement, OAuthTokenDestinations,
    OpaqueStateHash, PkceVerifierHash,
};
use brassclaw_host_api::SecretHandle;
use brassclaw_secrets::{SecretCustodySource, SecretCustodyState, SecretStore};
use chrono::{Duration, Utc};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(crate) enum Backend {
    NativePostgres,
    #[cfg(feature = "skills-db")]
    RecordFilesystem,
}
impl Backend {
    async fn remove_account(
        self,
        pool: &brassclaw_pg::PgPool,
        id: brassclaw_auth::CredentialAccountId,
    ) {
        let client = pool.get().await.unwrap();
        let count = match self {
            Self::NativePostgres => client
                .execute(
                    "DELETE FROM brassclaw_product_auth_accounts WHERE id=$1",
                    &[&id.to_string()],
                )
                .await
                .unwrap(),
            #[cfg(feature = "skills-db")]
            Self::RecordFilesystem => client
                .execute(
                    "DELETE FROM root_filesystem_entries WHERE path LIKE $1",
                    &[&format!("%/accounts/{id}.json")],
                )
                .await
                .unwrap(),
        };
        assert_eq!(count, 1, "actually remove the selected account record");
    }
}

fn callback(original: &OAuthSetupOriginal) -> OAuthCallbackPrepared {
    let definition = &original.definition;
    OAuthCallbackPrepared {
        original: OAuthCallbackOriginal {
            scope: definition.scope.clone(),
            flow_id: definition.flow_id,
            provider: definition.provider.clone(),
            account_label: definition.account_label.clone(),
            requested_scopes: definition.requested_scopes.clone(),
            opaque_state_hash: definition.opaque_state_hash.clone(),
            pkce_verifier_hash: definition.pkce_verifier_hash.clone(),
            authorization_code_hash: AuthorizationCodeHash::new("c".repeat(64)).unwrap(),
        },
        destinations: OAuthTokenDestinations {
            access_secret: SecretHandle::new(format!("setup-output-access-{}", definition.flow_id))
                .unwrap(),
            refresh_secret: SecretHandle::new(format!(
                "setup-output-refresh-{}",
                definition.flow_id
            ))
            .unwrap(),
        },
    }
}

pub(crate) async fn verify_storage(
    first: Arc<dyn AuthFlowManager>,
    second: Arc<dyn AuthFlowManager>,
    accounts: Arc<dyn CredentialAccountService>,
    secrets: Arc<dyn SecretStore>,
    pool: &brassclaw_pg::PgPool,
    backend: Backend,
    scope: AuthProductScope,
) {
    for mode in ["live", "expired", "cancel", "marker", "bound", "legacy"] {
        let now = Utc::now();
        let id = AuthFlowId::new();
        let mut definition = OAuthSetupDefinition {
            scope: scope.clone(),
            flow_id: id,
            provider: AuthProviderId::new("google").unwrap(),
            account_label: CredentialAccountLabel::new("Google original storage").unwrap(),
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
        };
        if mode == "bound" {
            let access = SecretHandle::new(format!("setup-account-access-{id}")).unwrap();
            secrets
                .put(
                    scope.resource.clone(),
                    access.clone(),
                    "private account access sentinel".into(),
                )
                .await
                .unwrap();
            let account = accounts
                .create_account(NewCredentialAccount {
                    scope: scope.clone(),
                    provider: definition.provider.clone(),
                    label: definition.account_label.clone(),
                    status: CredentialAccountStatus::Configured,
                    ownership: CredentialOwnership::UserReusable,
                    owner_extension: None,
                    granted_extensions: vec![],
                    access_secret: Some(access),
                    access_expires_at: None,
                    refresh_secret: None,
                    scopes: vec![],
                })
                .await
                .unwrap();
            definition.update_binding = Some(CredentialAccountUpdateBinding {
                account_id: account.id,
                ownership: account.ownership,
                owner_extension: account.owner_extension.clone(),
                granted_extensions: account.granted_extensions.clone(),
            });
            definition.update_account_snapshot = Some(account);
        }
        if mode == "expired" {
            definition.expires_at = now + Duration::seconds(2);
        }
        let mut original = OAuthSetupOriginal {
            definition,
            preparation: OAuthSetupMaterialReference {
                handle: SecretHandle::new(format!("oauth-setup-original-{id}")).unwrap(),
                reference: "not-an-authenticated-generation".into(),
            },
        };
        assert!(
            first
                .prepare_oauth_setup(original.clone(), None)
                .await
                .is_err()
        );
        assert!(first.get_flow(&scope, id).await.unwrap().is_none());
        let source = secrets
            .put_original(
                scope.resource.clone(),
                original.preparation.handle.clone(),
                "private setup fixture: client and PKCE remain inside the encrypted owner".into(),
                original.definition.preparation_context(),
            )
            .await
            .unwrap()
            .unwrap();
        original.preparation.reference = source.reference.clone();
        let mut forged = original.clone();
        forged.preparation.reference = "a-different-generation".into();
        assert!(
            first
                .prepare_oauth_setup(forged, Some(OAuthSetupRetirement::Canceled))
                .await
                .is_err()
        );
        let mut changed = original.clone();
        changed.definition.account_label =
            CredentialAccountLabel::new("another original label").unwrap();
        assert!(first.prepare_oauth_setup(changed, None).await.is_err());
        assert!(first.get_flow(&scope, id).await.unwrap().is_none());

        if mode == "marker" {
            let handle = original.definition.exchange_use_handle().unwrap();
            secrets
                .put_retained_from_original(
                    scope.resource.clone(),
                    handle.clone(),
                    SecretCustodySource {
                        scope: scope.resource.clone(),
                        handle: original.preparation.handle.clone(),
                        reference: original.preparation.reference.clone(),
                    },
                )
                .await
                .unwrap()
                .unwrap();
            for retired in [false, true] {
                if retired {
                    assert!(secrets.delete(&scope.resource, &handle).await.unwrap());
                }
                assert!(
                    first
                        .prepare_oauth_setup(original.clone(), None)
                        .await
                        .is_err()
                );
                assert!(
                    first
                        .prepare_oauth_setup(original.clone(), Some(OAuthSetupRetirement::Canceled))
                        .await
                        .is_err()
                );
                assert!(first.get_flow(&scope, id).await.unwrap().is_none());
            }
            continue;
        }
        if mode == "legacy" {
            let old = first
                .create_flow(NewAuthFlow {
                    requirement_setup: None,
                    id: Some(id),
                    scope: scope.clone(),
                    kind: brassclaw_auth::AuthFlowKind::IntegrationCredential,
                    provider: original.definition.provider.clone(),
                    challenge: brassclaw_auth::AuthChallenge::OAuthUrl {
                        authorization_url: original.definition.authorization_url.clone(),
                        expires_at: original.definition.expires_at,
                    },
                    continuation: AuthContinuationRef::SetupOnly,
                    update_binding: None,
                    opaque_state_hash: Some(original.definition.opaque_state_hash.clone()),
                    pkce_verifier_hash: Some(original.definition.pkce_verifier_hash.clone()),
                    expires_at: original.definition.expires_at,
                })
                .await
                .unwrap();
            assert!(first.read_oauth_setup(&scope, id).await.unwrap().is_none());
            assert!(
                first
                    .prepare_oauth_setup(original.clone(), None)
                    .await
                    .is_err()
            );
            assert_eq!(first.get_flow(&scope, id).await.unwrap().unwrap(), old);
            continue;
        }
        if mode == "expired"
            && let Ok(remaining) = (original.definition.expires_at - Utc::now()).to_std()
        {
            tokio::time::sleep(remaining).await;
        }
        if mode == "bound" {
            backend
                .remove_account(
                    pool,
                    original
                        .definition
                        .update_binding
                        .as_ref()
                        .unwrap()
                        .account_id,
                )
                .await;
        }
        let retirement = (mode == "cancel").then_some(OAuthSetupRetirement::Canceled);
        let (a, b) = tokio::join!(
            first.prepare_oauth_setup(original.clone(), retirement),
            second.prepare_oauth_setup(original.clone(), retirement)
        );
        let selected = a.unwrap();
        assert_eq!(selected, b.unwrap());
        assert_eq!(selected.original, original);
        assert_eq!(selected.flow.created_at, original.definition.created_at);
        assert_eq!(selected.flow.expires_at, original.definition.expires_at);
        assert_eq!(
            second.read_oauth_setup(&scope, id).await.unwrap().unwrap(),
            selected
        );
        if mode == "live" || mode == "bound" {
            assert_eq!(selected.flow.status, AuthFlowStatus::AwaitingUser);
            let prepared = callback(&original);
            let mut wrong = prepared.clone();
            wrong.original.account_label =
                CredentialAccountLabel::new("retargeted callback").unwrap();
            assert_eq!(
                first.claim_oauth_callback(&scope, wrong).await.unwrap_err(),
                AuthProductError::BackendConflict
            );
            if mode == "bound" {
                assert!(first.claim_oauth_callback(&scope, prepared).await.is_err());
                assert_eq!(
                    first.read_oauth_setup(&scope, id).await.unwrap().unwrap(),
                    selected
                );
                let canceled = first.cancel_flow(&scope, id).await.unwrap();
                assert_eq!(canceled.status, AuthFlowStatus::Canceled);
            } else {
                assert!(matches!(
                    first
                        .claim_oauth_callback(&scope, prepared.clone())
                        .await
                        .unwrap(),
                    OAuthCallbackAdmission::Dispatch(_)
                ));
                assert!(matches!(
                    second.claim_oauth_callback(&scope, prepared).await.unwrap(),
                    OAuthCallbackAdmission::Original(_)
                ));
            }
        } else {
            assert_eq!(
                selected.flow.status,
                if mode == "expired" {
                    AuthFlowStatus::Expired
                } else {
                    AuthFlowStatus::Canceled
                }
            );
            assert!(selected.flow.challenge.is_none());
            assert!(
                first
                    .claim_oauth_callback(&scope, callback(&original))
                    .await
                    .is_err()
            );
        }
        let before = second.read_oauth_setup(&scope, id).await.unwrap().unwrap();
        assert_eq!(
            first
                .prepare_oauth_setup(original.clone(), None)
                .await
                .unwrap(),
            before
        );
        let mut wrong = original.clone();
        wrong.preparation.reference = "replacement generation".into();
        assert!(first.prepare_oauth_setup(wrong, None).await.is_err());
        if matches!(
            before.flow.status,
            AuthFlowStatus::Expired | AuthFlowStatus::Canceled
        ) {
            assert!(
                secrets
                    .delete(&scope.resource, &original.preparation.handle)
                    .await
                    .unwrap()
            );
            assert!(matches!(
                secrets
                    .custody(&scope.resource, &original.preparation.handle)
                    .await
                    .unwrap()
                    .unwrap()
                    .state,
                SecretCustodyState::RetiredOriginal { .. }
            ));
            assert_eq!(
                second.read_oauth_setup(&scope, id).await.unwrap().unwrap(),
                before
            );
            assert_eq!(
                first.prepare_oauth_setup(original, None).await.unwrap(),
                before
            );
        }
    }
}
