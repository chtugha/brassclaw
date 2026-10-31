use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_auth::{
    AuthChallenge, AuthContinuationRef, AuthErrorCode, AuthFlowRecord, AuthFlowStatus,
    AuthProductError, CredentialAccountLabel, CredentialAccountLookupRequest,
    CredentialAccountUpdateBinding, scope_matches,
};
use brassclaw_product_workflow::{
    ExtensionCredentialPrepareRequest, ExtensionCredentialReferenceRequest,
    ExtensionCredentialSetupService, ExtensionCredentialStatusRequest,
    ExtensionCredentialSubmitRequest, ExtensionCredentialTarget, ExtensionManualTokenCompletion,
    ExtensionManualTokenOriginal, ExtensionManualTokenSetupReceipt, RebornServicesError,
    RebornServicesErrorCode, RebornServicesErrorKind, extension_manual_token_label,
};
use chrono::{Duration as ChronoDuration, Utc};

use crate::{
    RebornManualTokenSetupRequest, RebornManualTokenSubmitRequest, RebornProductAuthServices,
};

const EXTENSION_CREDENTIAL_SETUP_TTL_SECONDS: i64 = 300;

#[derive(Clone)]
pub(crate) struct ProductAuthExtensionCredentialSetup {
    product_auth: Arc<RebornProductAuthServices>,
}

impl ProductAuthExtensionCredentialSetup {
    pub(crate) fn new(product_auth: Arc<RebornProductAuthServices>) -> Self {
        Self { product_auth }
    }
}

#[async_trait]
impl ExtensionCredentialSetupService for ProductAuthExtensionCredentialSetup {
    async fn credential_status(
        &self,
        request: ExtensionCredentialStatusRequest,
    ) -> Result<Option<brassclaw_auth::CredentialAccountProjection>, RebornServicesError> {
        let records = self.product_auth.credential_account_record_source();
        let selected = records
            .requirement_selection(&request.scope, &request.requirement)
            .await
            .map_err(|error| map_auth_error(error.into()))?;
        let Some(selected) = selected else {
            return Ok(None);
        };
        // The head reader authenticates original completed flow custody. Current
        // eligibility is separate; a newer account is not this old selection.
        let account = records
            .account_record(&selected.account().scope, selected.account().id)
            .await
            .map_err(|error| map_auth_error(error.into()))?;
        let Some(account) = account else {
            return Ok(None);
        };
        if account != *selected.account()
            || account.effective_status() != brassclaw_auth::CredentialAccountStatus::Configured
            || !account.is_authorized_for_requester(Some(request.requirement.requester_extension()))
        {
            return Ok(None);
        }
        Ok(Some(account.projection()))
    }

    async fn recover_manual_token(
        &self,
        request: ExtensionCredentialReferenceRequest,
    ) -> Result<Option<ExtensionManualTokenOriginal>, RebornServicesError> {
        let Some(flow) = self.recover(&request).await? else {
            return Ok(None);
        };
        let target =
            flow.update_binding
                .as_ref()
                .map_or(ExtensionCredentialTarget::New {}, |binding| {
                    ExtensionCredentialTarget::Existing {
                        account_id: binding.account_id,
                    }
                });
        Ok(Some(ExtensionManualTokenOriginal {
            receipt: setup_receipt(&request, flow)?,
            target,
        }))
    }

    async fn prepare_manual_token(
        &self,
        request: ExtensionCredentialPrepareRequest,
    ) -> Result<ExtensionManualTokenSetupReceipt, RebornServicesError> {
        let reference = ExtensionCredentialReferenceRequest {
            scope: request.scope.clone(),
            requester_extension: request.requester_extension.clone(),
            requirement_name: request.requirement_name.clone(),
            identity: request.identity,
        };
        if let Some(flow) = self.recover(&reference).await? {
            // A repeat uses the original prechallenge binding, not today's
            // account selection or mutable account state.
            let same_target = match (&request.target, &flow.update_binding) {
                (ExtensionCredentialTarget::New {}, None) => true,
                (ExtensionCredentialTarget::Existing { account_id }, Some(binding)) => {
                    *account_id == binding.account_id
                }
                _ => false,
            };
            if !same_target {
                return Err(original_conflict());
            }
            return setup_receipt(&reference, flow);
        }
        let requirement = request
            .candidate_requirement
            .ok_or_else(invalid_auth_setup_request)?;
        if requirement.requester_extension() != &request.requester_extension {
            return Err(invalid_auth_setup_request());
        }
        let brassclaw_host_api::RuntimeCredentialRequirementSource::ProductAuthAccount {
            provider,
            setup: brassclaw_host_api::RuntimeCredentialAccountSetup::ManualToken,
        } = &requirement.requirement().source
        else {
            return Err(invalid_auth_setup_request());
        };
        let provider = brassclaw_auth::AuthProviderId::new(provider.as_str())
            .map_err(|_| invalid_auth_setup_request())?;
        if request.requirement_name.starts_with("sha256:")
            && requirement
                .reference()
                .map_err(|_| invalid_auth_setup_request())?
                != request.requirement_name
        {
            return Err(invalid_auth_setup_request());
        }
        let label = CredentialAccountLabel::new(extension_manual_token_label(
            request.requester_extension.as_str(),
            provider.as_str(),
            &request.requirement_name,
        ))
        .map_err(|_| invalid_auth_setup_request())?;
        let previous = self
            .product_auth
            .credential_account_record_source()
            .requirement_selection(&request.scope, &requirement)
            .await
            .map_err(|error| map_auth_error(error.into()))?;
        let setup =
            brassclaw_auth::CredentialRequirementSetup::new(&request.scope, requirement, previous)
                .map_err(|error| map_auth_error(error.into()))?;
        let mut setup = RebornManualTokenSetupRequest::new(
            request.scope.clone(),
            provider.clone(),
            label,
            AuthContinuationRef::SetupOnly,
            Utc::now() + ChronoDuration::seconds(EXTENSION_CREDENTIAL_SETUP_TTL_SECONDS),
        )
        .with_requirement_setup(setup);
        if let ExtensionCredentialTarget::Existing { account_id } = request.target {
            let account = self
                .product_auth
                .credential_account_service()
                .get_account(CredentialAccountLookupRequest::new(
                    request.scope.clone(),
                    account_id,
                ))
                .await
                .map_err(|error| map_auth_error(error.into()))?
                .ok_or_else(|| map_auth_error(AuthProductError::CredentialMissing.into()))?;
            if !scope_matches(&request.scope, &account.scope)
                || account.provider != provider
                || !account.is_authorized_for_requester(Some(&request.requester_extension))
            {
                return Err(map_auth_error(AuthProductError::CrossScopeDenied.into()));
            }
            setup = setup.with_update_binding(CredentialAccountUpdateBinding::from_projection(
                &account.projection(),
            ));
        }
        let original = self
            .product_auth
            .request_original_manual_token_setup(request.identity, setup)
            .await
            .map_err(map_auth_error)?;
        Ok(ExtensionManualTokenSetupReceipt {
            identity: original.identity,
            invocation_id: request.scope.resource.invocation_id,
            status: original.status,
            provider: original.challenge.provider,
            label: original.challenge.label,
            expires_at: original.challenge.expires_at,
        })
    }

    async fn submit_manual_token(
        &self,
        request: ExtensionCredentialSubmitRequest,
    ) -> Result<ExtensionManualTokenCompletion, RebornServicesError> {
        let reference = ExtensionCredentialReferenceRequest {
            scope: request.scope.clone(),
            requester_extension: request.requester_extension,
            requirement_name: request.requirement_name,
            identity: request.identity,
        };
        self.require_original(&reference).await?;
        let submitted = self
            .product_auth
            .submit_original_manual_token(
                request.identity,
                RebornManualTokenSubmitRequest::new(
                    request.scope,
                    request.identity.interaction_id,
                    request.secret,
                ),
            )
            .await
            .map_err(map_auth_error)?;
        Ok(completion(&reference, submitted))
    }

    async fn continue_manual_token(
        &self,
        request: ExtensionCredentialReferenceRequest,
    ) -> Result<ExtensionManualTokenCompletion, RebornServicesError> {
        self.require_original(&request).await?;
        let submitted = self
            .product_auth
            .continue_original_manual_token(&request.scope, request.identity)
            .await
            .map_err(map_auth_error)?;
        Ok(completion(&request, submitted))
    }

    async fn cancel_manual_token(
        &self,
        request: ExtensionCredentialReferenceRequest,
    ) -> Result<ExtensionManualTokenSetupReceipt, RebornServicesError> {
        let original = self.require_original(&request).await?;
        if original.status == AuthFlowStatus::AwaitingUser {
            self.product_auth
                .abandon_manual_token(&request.scope, request.identity.interaction_id)
                .await
                .map_err(map_auth_error)?;
        }
        // Publication may have won a concurrent cancel. Report actual state;
        // never claim a completed original was canceled.
        setup_receipt(&request, self.require_original(&request).await?)
    }
}

impl ProductAuthExtensionCredentialSetup {
    async fn recover(
        &self,
        request: &ExtensionCredentialReferenceRequest,
    ) -> Result<Option<AuthFlowRecord>, RebornServicesError> {
        let flow = self
            .product_auth
            .recover_original_manual_token_setup(&request.scope, request.identity)
            .await
            .map_err(map_auth_error)?;
        if let Some(flow) = &flow {
            let expected = extension_manual_token_label(
                request.requester_extension.as_str(),
                flow.provider.as_str(),
                &request.requirement_name,
            );
            if flow.continuation != AuthContinuationRef::SetupOnly
                || !matches!(&flow.challenge, Some(AuthChallenge::ManualTokenRequired { provider, label, .. })
                    if provider == &flow.provider && label.as_str() == expected)
            {
                return Err(original_conflict());
            }
            if let Some(setup) = self
                .product_auth
                .flow_manager()
                .requirement_setup(&request.scope, flow.id)
                .await
                .map_err(|error| map_auth_error(error.into()))?
            {
                setup
                    .verify_flow(flow)
                    .map_err(|error| map_auth_error(error.into()))?;
                if setup.requirement().requester_extension() != &request.requester_extension
                    || (request.requirement_name.starts_with("sha256:")
                        && setup
                            .requirement()
                            .reference()
                            .map_err(|_| original_conflict())?
                            != request.requirement_name)
                {
                    return Err(original_conflict());
                }
            }
            // Legacy originals without usage custody remain recoverable but
            // never gain a new requirement selection or readiness grant here.
        }
        Ok(flow)
    }

    async fn require_original(
        &self,
        request: &ExtensionCredentialReferenceRequest,
    ) -> Result<AuthFlowRecord, RebornServicesError> {
        self.recover(request)
            .await?
            .ok_or_else(|| map_auth_error(AuthProductError::UnknownOrExpiredFlow.into()))
    }
}

fn setup_receipt(
    request: &ExtensionCredentialReferenceRequest,
    flow: AuthFlowRecord,
) -> Result<ExtensionManualTokenSetupReceipt, RebornServicesError> {
    let Some(AuthChallenge::ManualTokenRequired {
        provider,
        label,
        expires_at,
        ..
    }) = flow.challenge
    else {
        return Err(original_conflict());
    };
    Ok(ExtensionManualTokenSetupReceipt {
        identity: request.identity,
        invocation_id: request.scope.resource.invocation_id,
        status: if flow.status == AuthFlowStatus::AwaitingUser && expires_at <= Utc::now() {
            AuthFlowStatus::Expired
        } else {
            flow.status
        },
        provider,
        label,
        expires_at,
    })
}

fn completion(
    request: &ExtensionCredentialReferenceRequest,
    submitted: crate::RebornManualTokenSubmitResponse,
) -> ExtensionManualTokenCompletion {
    ExtensionManualTokenCompletion {
        identity: request.identity,
        invocation_id: request.scope.resource.invocation_id,
        credential_ref: submitted.account_id,
        status: submitted.status,
        continuation: submitted.continuation,
    }
}

fn original_conflict() -> RebornServicesError {
    services_error(
        RebornServicesErrorCode::Conflict,
        RebornServicesErrorKind::BlockedAuthentication,
        409,
        false,
    )
}

fn map_auth_error(error: crate::RebornAuthProductError) -> RebornServicesError {
    if error.conflict {
        return original_conflict();
    }
    match error.code {
        AuthErrorCode::InvalidRequest | AuthErrorCode::MalformedCallback => {
            invalid_auth_setup_request()
        }
        AuthErrorCode::CrossScopeDenied => services_error(
            RebornServicesErrorCode::Forbidden,
            RebornServicesErrorKind::ParticipantDenied,
            403,
            false,
        ),
        AuthErrorCode::BackendUnavailable | AuthErrorCode::MalformedConfig => services_error(
            RebornServicesErrorCode::Unavailable,
            RebornServicesErrorKind::ServiceUnavailable,
            503,
            error.retryable,
        ),
        AuthErrorCode::AccountSelectionRequired => services_error(
            RebornServicesErrorCode::Conflict,
            RebornServicesErrorKind::BlockedAuthentication,
            409,
            false,
        ),
        AuthErrorCode::CredentialMissing | AuthErrorCode::UnknownOrExpiredFlow => services_error(
            RebornServicesErrorCode::NotFound,
            RebornServicesErrorKind::BlockedAuthentication,
            404,
            false,
        ),
        AuthErrorCode::Canceled | AuthErrorCode::FlowAlreadyTerminal => original_conflict(),
        AuthErrorCode::ProviderDenied
        | AuthErrorCode::TokenExchangeFailed
        | AuthErrorCode::RefreshFailed => services_error(
            RebornServicesErrorCode::Internal,
            RebornServicesErrorKind::BlockedAuthentication,
            500,
            error.retryable,
        ),
    }
}

fn invalid_auth_setup_request() -> RebornServicesError {
    services_error(
        RebornServicesErrorCode::InvalidRequest,
        RebornServicesErrorKind::Validation,
        400,
        false,
    )
}

fn services_error(
    code: RebornServicesErrorCode,
    kind: RebornServicesErrorKind,
    status_code: u16,
    retryable: bool,
) -> RebornServicesError {
    RebornServicesError {
        code,
        kind,
        status_code,
        retryable,
        field: None,
        validation_code: None,
    }
}

/// Complete declaration fixture for the real metadata/publication callers.
#[cfg(test)]
pub(crate) fn test_requirement(
    provider: &str,
    extension: &str,
    handle: &str,
    scopes: &[&str],
) -> brassclaw_host_api::RuntimeCredentialAccountRequirement {
    use brassclaw_host_api::*;
    RuntimeCredentialAccountRequirement::new(
        ExtensionId::new(extension).unwrap(),
        RuntimeCredentialRequirement {
            handle: SecretHandle::new(handle).unwrap(),
            source: RuntimeCredentialRequirementSource::ProductAuthAccount {
                provider: RuntimeCredentialAccountProviderId::new(provider).unwrap(),
                setup: if scopes.is_empty() {
                    RuntimeCredentialAccountSetup::ManualToken
                } else {
                    RuntimeCredentialAccountSetup::OAuth {
                        scopes: scopes.iter().map(|s| (*s).into()).collect(),
                    }
                },
            },
            provider_scopes: scopes.iter().map(|s| (*s).into()).collect(),
            audience: NetworkTargetPattern {
                scheme: None,
                host_pattern: "api.example.test".into(),
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

/// Actual adapter contract shared by encrypted native/FS and in-memory owners.
#[cfg(test)]
pub(crate) async fn verify_extension_requirement_readiness(
    facade: RebornProductAuthServices,
    mut scope: brassclaw_auth::AuthProductScope,
) {
    scope.surface = brassclaw_auth::AuthSurface::Web;
    use brassclaw_auth::ManualTokenSetupIdentity;
    use brassclaw_host_api::{RuntimeCredentialAccountRequirement, RuntimeCredentialTarget};
    let facade = Arc::new(facade);
    let adapter = ProductAuthExtensionCredentialSetup::new(facade.clone());
    let usage = test_requirement("usage-provider", "usage-extension", "access", &[]);
    let mut different = usage.requirement().clone();
    different.target = RuntimeCredentialTarget::Header {
        name: "X-Other-Credential".into(),
        prefix: None,
    };
    let other =
        RuntimeCredentialAccountRequirement::new(usage.requester_extension().clone(), different)
            .unwrap();
    let status =
        |requirement: &RuntimeCredentialAccountRequirement| ExtensionCredentialStatusRequest {
            scope: scope.clone(),
            requirement: requirement.clone(),
        };
    let prepare =
        |requirement: &RuntimeCredentialAccountRequirement| ExtensionCredentialPrepareRequest {
            scope: scope.clone(),
            requester_extension: requirement.requester_extension().clone(),
            requirement_name: requirement.reference().unwrap(),
            candidate_requirement: Some(requirement.clone()),
            identity: ManualTokenSetupIdentity::default(),
            target: ExtensionCredentialTarget::New {},
        };
    let first = prepare(&usage);
    let receipt = adapter.prepare_manual_token(first.clone()).await.unwrap();
    assert!(
        adapter
            .credential_status(status(&usage))
            .await
            .unwrap()
            .is_none()
    );
    let submit = |request: &ExtensionCredentialPrepareRequest| ExtensionCredentialSubmitRequest {
        scope: scope.clone(),
        requester_extension: request.requester_extension.clone(),
        requirement_name: request.requirement_name.clone(),
        identity: request.identity,
        secret: "actual original usage credential".into(),
    };
    let original = adapter.submit_manual_token(submit(&first)).await.unwrap();
    assert_eq!(
        adapter
            .credential_status(status(&usage))
            .await
            .unwrap()
            .unwrap()
            .id,
        original.credential_ref
    );
    assert!(
        adapter
            .credential_status(status(&other))
            .await
            .unwrap()
            .is_none(),
        "same provider/handle is not another injection usage"
    );
    let second = prepare(&other);
    adapter.prepare_manual_token(second.clone()).await.unwrap();
    let successor = adapter.submit_manual_token(submit(&second)).await.unwrap();
    assert_ne!(original.credential_ref, successor.credential_ref);
    assert_eq!(
        adapter
            .credential_status(status(&usage))
            .await
            .unwrap()
            .unwrap()
            .id,
        original.credential_ref
    );
    assert_eq!(
        adapter
            .credential_status(status(&other))
            .await
            .unwrap()
            .unwrap()
            .id,
        successor.credential_ref
    );
    let changed_candidate = ExtensionCredentialPrepareRequest {
        candidate_requirement: Some(other.clone()),
        ..first.clone()
    };
    let recovered = adapter
        .prepare_manual_token(changed_candidate)
        .await
        .unwrap();
    assert_eq!(recovered.identity, receipt.identity);
    assert_eq!(recovered.expires_at, receipt.expires_at);
    assert_eq!(recovered.status, AuthFlowStatus::Completed);
    assert_eq!(
        facade
            .flow_manager()
            .requirement_setup(&scope, first.identity.flow_id)
            .await
            .unwrap()
            .unwrap()
            .requirement(),
        &usage
    );
    let reference = ExtensionCredentialReferenceRequest {
        scope: scope.clone(),
        requester_extension: first.requester_extension.clone(),
        requirement_name: first.requirement_name.clone(),
        identity: first.identity,
    };
    assert_eq!(
        adapter
            .continue_manual_token(reference.clone())
            .await
            .unwrap(),
        original
    );
    let wrong = ExtensionCredentialReferenceRequest {
        requirement_name: second.requirement_name,
        ..reference.clone()
    };
    assert_eq!(
        adapter
            .continue_manual_token(wrong)
            .await
            .unwrap_err()
            .status_code,
        409
    );
    let unknown = ExtensionCredentialReferenceRequest {
        identity: ManualTokenSetupIdentity::default(),
        ..reference
    };
    assert_eq!(
        adapter
            .continue_manual_token(unknown)
            .await
            .unwrap_err()
            .status_code,
        404
    );
    facade
        .credential_account_service()
        .update_status(
            &scope,
            original.credential_ref,
            brassclaw_auth::CredentialAccountStatus::Revoked,
        )
        .await
        .unwrap();
    assert!(
        adapter
            .credential_status(status(&usage))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        adapter
            .credential_status(status(&other))
            .await
            .unwrap()
            .unwrap()
            .id,
        successor.credential_ref
    );
}

/// Actual-time regression shared by Native/FS/Fake owner tests. A metadata
/// read is not permission to accept an expired raw input or replay an effect.
#[cfg(test)]
pub(crate) async fn verify_extension_metadata_expiry(
    facade: RebornProductAuthServices,
    mut scope: brassclaw_auth::AuthProductScope,
) -> (
    brassclaw_auth::AuthProductScope,
    brassclaw_auth::ManualTokenSetupIdentity,
    brassclaw_auth::ManualTokenSetupIdentity,
) {
    use brassclaw_auth::{AuthSurface, CredentialAccountListRequest, ManualTokenSetupIdentity};
    scope.surface = AuthSurface::Web;
    let facade = Arc::new(facade);
    let adapter = ProductAuthExtensionCredentialSetup::new(facade.clone());
    let provider = brassclaw_auth::AuthProviderId::new("expiry-manual").unwrap();
    let extension = brassclaw_host_api::ExtensionId::new("expiry-extension").unwrap();
    let label = "expiry-extension expiry-manual token";
    let expires_at = Utc::now() + ChronoDuration::seconds(5);
    let pending = ManualTokenSetupIdentity::default();
    let completed = ManualTokenSetupIdentity::default();
    let request = RebornManualTokenSetupRequest::new(
        scope.clone(),
        provider.clone(),
        CredentialAccountLabel::new(label).unwrap(),
        AuthContinuationRef::SetupOnly,
        expires_at,
    );
    let first = facade
        .request_original_manual_token_setup(pending, request.clone())
        .await
        .unwrap();
    facade
        .request_original_manual_token_setup(completed, request.clone())
        .await
        .unwrap();
    let published = facade
        .submit_original_manual_token(
            completed,
            RebornManualTokenSubmitRequest::new(
                scope.clone(),
                completed.interaction_id,
                "confirmed before expiry".into(),
            ),
        )
        .await
        .unwrap();
    // Wait for this actual retained timestamp, not a fake clock or altered store.
    let remaining = (expires_at - Utc::now()).to_std().unwrap_or_default();
    tokio::time::sleep(remaining + std::time::Duration::from_millis(10)).await;
    let retained = facade
        .recover_original_manual_token_setup(&scope, pending)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.id, pending.flow_id);
    assert_eq!(retained.scope, scope);
    assert_eq!(retained.expires_at, first.challenge.expires_at);
    assert_eq!(
        retained.status,
        AuthFlowStatus::AwaitingUser,
        "read-derived expiry does not mutate original state"
    );
    let expired = adapter
        .prepare_manual_token(ExtensionCredentialPrepareRequest {
            candidate_requirement: None,

            requirement_name: "token".into(),

            scope: scope.clone(),
            requester_extension: extension.clone(),
            identity: pending,
            target: ExtensionCredentialTarget::New {},
        })
        .await
        .unwrap();
    assert_eq!(expired.status, AuthFlowStatus::Expired);
    assert_eq!(expired.identity, pending);
    assert_eq!(expired.expires_at, expires_at);
    let mut later = request;
    later.expires_at += ChronoDuration::hours(1);
    assert_eq!(
        facade
            .request_original_manual_token_setup(pending, later)
            .await
            .unwrap_err()
            .code,
        AuthErrorCode::UnknownOrExpiredFlow
    );
    assert_eq!(
        adapter
            .submit_manual_token(ExtensionCredentialSubmitRequest {
                requirement_name: "token".into(),

                scope: scope.clone(),
                requester_extension: extension.clone(),
                identity: pending,
                secret: "must never publish after expiry".into(),
            })
            .await
            .unwrap_err()
            .status_code,
        404
    );
    assert!(
        adapter
            .continue_manual_token(ExtensionCredentialReferenceRequest {
                requirement_name: "token".into(),

                scope: scope.clone(),
                requester_extension: extension,
                identity: pending,
            })
            .await
            .is_err()
    );
    assert_eq!(
        facade
            .continue_original_manual_token(&scope, completed)
            .await
            .unwrap(),
        published,
        "expiry must not erase confirmed effect evidence or replay raw publication"
    );
    let completed_metadata = facade
        .recover_original_manual_token_setup(&scope, completed)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed_metadata.status, AuthFlowStatus::Completed);
    assert_eq!(
        completed_metadata.credential_account_id,
        Some(published.account_id)
    );
    let accounts = facade
        .credential_account_service()
        .list_accounts(CredentialAccountListRequest::new(scope.clone(), provider))
        .await
        .unwrap();
    assert_eq!(
        accounts.accounts.len(),
        1,
        "expired input creates no account"
    );
    assert_eq!(accounts.accounts[0].id, published.account_id);
    (scope, pending, completed)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::ProductAuthExtensionCredentialSetup;
    use async_trait::async_trait;
    use brassclaw_auth::{
        AuthContinuationEvent, AuthProductError, AuthProductScope, AuthProviderId, AuthSurface,
        CredentialAccountLabel, CredentialAccountService, CredentialAccountStatus,
        CredentialOwnership, InMemoryAuthProductServices, NewCredentialAccount, ProviderScope,
    };
    use brassclaw_host_api::{
        ExtensionId, InvocationId, ResourceScope, SecretHandle, TenantId, UserId,
    };
    use brassclaw_product_workflow::{
        ExtensionCredentialSetupService, ExtensionCredentialStatusRequest,
    };

    use crate::{RebornAuthContinuationDispatcher, RebornProductAuthServices};

    struct NoopDispatcher;

    #[async_trait]
    impl RebornAuthContinuationDispatcher for NoopDispatcher {
        async fn dispatch_auth_continuation(
            &self,
            _event: AuthContinuationEvent,
        ) -> Result<(), AuthProductError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn credential_status_does_not_report_ambiguous_accounts_as_connected() {
        // Readiness must use the same explicit-selection boundary as runtime.
        // A newly created same-provider account is not an implicit binding.
        let shared = Arc::new(InMemoryAuthProductServices::new());
        let service = ProductAuthExtensionCredentialSetup::new(Arc::new(
            RebornProductAuthServices::from_shared(shared.clone(), Arc::new(NoopDispatcher)),
        ));
        let scope = test_scope();
        seed_account(&shared, scope.clone(), "notion primary").await;
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        seed_account(&shared, scope.clone(), "notion secondary").await;

        let status = service
            .credential_status(ExtensionCredentialStatusRequest {
                scope,
                requirement: super::test_requirement("notion", "notion", "api_token", &[]),
            })
            .await
            .expect("status lookup should not block setup");

        assert!(
            status.is_none(),
            "ambiguity must not report either account as connected"
        );
    }

    #[tokio::test]
    async fn credential_status_treats_unauthorized_accounts_as_reconnectable() {
        let shared = Arc::new(InMemoryAuthProductServices::new());
        let service = ProductAuthExtensionCredentialSetup::new(Arc::new(
            RebornProductAuthServices::from_shared(shared.clone(), Arc::new(NoopDispatcher)),
        ));
        let scope = test_scope();
        shared
            .create_account(NewCredentialAccount {
                scope: scope.clone(),
                provider: AuthProviderId::new("notion").expect("provider"),
                label: CredentialAccountLabel::new("admin notion").expect("label"),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::SharedAdminManaged,
                owner_extension: None,
                granted_extensions: vec![ExtensionId::new("other-extension").expect("extension")],
                access_secret: Some(SecretHandle::new("admin-notion-access").expect("secret")),
                access_expires_at: None,
                refresh_secret: None,
                scopes: Vec::new(),
            })
            .await
            .expect("seed admin account");

        let status = service
            .credential_status(ExtensionCredentialStatusRequest {
                scope,
                requirement: super::test_requirement("notion", "notion", "api_token", &[]),
            })
            .await
            .expect("status lookup should not block setup");

        assert!(status.is_none());
    }

    #[tokio::test]
    async fn credential_status_finds_callback_surface_google_oauth_account_for_gsuite_extensions() {
        let shared = Arc::new(InMemoryAuthProductServices::new());
        let service = ProductAuthExtensionCredentialSetup::new(Arc::new(
            RebornProductAuthServices::from_shared(shared.clone(), Arc::new(NoopDispatcher)),
        ));
        let ui_scope = test_scope();
        let callback_scope =
            AuthProductScope::new(ui_scope.resource.clone(), AuthSurface::Callback);
        let account = shared
            .create_account(NewCredentialAccount {
                scope: callback_scope,
                provider: AuthProviderId::new("google").expect("provider"),
                label: CredentialAccountLabel::new("work google").expect("label"),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(SecretHandle::new("google-access").expect("secret")),
                access_expires_at: None,
                refresh_secret: None,
                scopes: vec![
                    ProviderScope::new("https://www.googleapis.com/auth/gmail.modify")
                        .expect("gmail scope"),
                    ProviderScope::new("https://www.googleapis.com/auth/calendar.events")
                        .expect("calendar scope"),
                ],
            })
            .await
            .expect("seed google account");

        for (extension, scope) in [
            ("gmail", "https://www.googleapis.com/auth/gmail.modify"),
            (
                "google-calendar",
                "https://www.googleapis.com/auth/calendar.events",
            ),
        ] {
            use brassclaw_auth::{
                AuthChallenge, AuthContinuationRef, AuthFlowKind, AuthFlowManager,
                CredentialRequirementSetup, CredentialSelectionInput, NewAuthFlow,
            };
            let requirement =
                super::test_requirement("google", extension, "access_token", &[scope]);
            assert!(
                service
                    .credential_status(ExtensionCredentialStatusRequest {
                        scope: ui_scope.clone(),
                        requirement: requirement.clone(),
                    })
                    .await
                    .unwrap()
                    .is_none(),
                "a compatible provider account alone is not a usage binding"
            );
            let flow = shared
                .create_flow(NewAuthFlow {
                    requirement_setup: Some(Box::new(
                        CredentialRequirementSetup::new(&account.scope, requirement.clone(), None)
                            .unwrap(),
                    )),
                    id: None,
                    scope: account.scope.clone(),
                    kind: AuthFlowKind::IntegrationCredential,
                    provider: account.provider.clone(),
                    challenge: AuthChallenge::AccountSelectionRequired {
                        provider: account.provider.clone(),
                        accounts: vec![account.projection()],
                    },
                    continuation: AuthContinuationRef::SetupOnly,
                    update_binding: None,
                    opaque_state_hash: None,
                    pkce_verifier_hash: None,
                    expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
                })
                .await
                .unwrap();
            shared
                .complete_credential_selection(
                    &account.scope,
                    CredentialSelectionInput {
                        flow_id: flow.id,
                        credential_account_id: account.id,
                    },
                )
                .await
                .unwrap();
            let status = service
                .credential_status(ExtensionCredentialStatusRequest {
                    scope: ui_scope.clone(),
                    requirement,
                })
                .await
                .expect("status lookup should succeed");

            assert!(
                status.is_some(),
                "{extension} should see callback-surface Google OAuth account as configured"
            );
        }
    }

    #[tokio::test]
    async fn extension_original_adapter_does_not_create_on_reference_or_use_current_account_on_retry()
     {
        use brassclaw_auth::{AuthFlowStatus, ManualTokenSetupIdentity};
        use brassclaw_product_workflow::{
            ExtensionCredentialPrepareRequest, ExtensionCredentialReferenceRequest,
            ExtensionCredentialSubmitRequest, ExtensionCredentialTarget,
        };
        let shared = Arc::new(InMemoryAuthProductServices::new());
        let service = ProductAuthExtensionCredentialSetup::new(Arc::new(
            RebornProductAuthServices::from_shared(shared.clone(), Arc::new(NoopDispatcher)),
        ));
        let mut scope = test_scope();
        scope.surface = AuthSurface::Web;
        let request = ExtensionCredentialPrepareRequest {
            candidate_requirement: Some(crate::webui_extension_credentials::test_requirement(
                (AuthProviderId::new("notion").unwrap()).as_str(),
                (ExtensionId::new("notion").unwrap()).as_str(),
                "api_token",
                &[],
            )),

            requirement_name: "api_token".into(),

            scope: scope.clone(),
            requester_extension: ExtensionId::new("notion").unwrap(),
            identity: ManualTokenSetupIdentity::default(),
            target: ExtensionCredentialTarget::New {},
        };
        let reference = ExtensionCredentialReferenceRequest {
            requirement_name: request.requirement_name.clone(),

            scope: scope.clone(),
            requester_extension: request.requester_extension.clone(),
            identity: request.identity,
        };
        assert_eq!(
            service
                .continue_manual_token(reference.clone())
                .await
                .unwrap_err()
                .status_code,
            404
        );
        assert_eq!(
            service
                .cancel_manual_token(reference.clone())
                .await
                .unwrap_err()
                .status_code,
            404
        );
        assert!(shared.flow_records_snapshot().is_empty());
        let first = service.prepare_manual_token(request.clone()).await.unwrap();
        let submitted = service
            .submit_manual_token(ExtensionCredentialSubmitRequest {
                requirement_name: request.requirement_name.clone(),

                scope: scope.clone(),
                requester_extension: request.requester_extension.clone(),
                identity: request.identity,
                secret: "original token ü".into(),
            })
            .await
            .unwrap();
        shared
            .update_status(
                &scope,
                submitted.credential_ref,
                CredentialAccountStatus::Revoked,
            )
            .await
            .unwrap();
        let repeated = service.prepare_manual_token(request.clone()).await.unwrap();
        assert_eq!(repeated.expires_at, first.expires_at);
        assert_eq!(repeated.status, AuthFlowStatus::Completed);
        assert_eq!(
            service
                .continue_manual_token(reference.clone())
                .await
                .unwrap(),
            submitted
        );
        assert_eq!(
            service
                .submit_manual_token(ExtensionCredentialSubmitRequest {
                    requirement_name: request.requirement_name.clone(),

                    scope: scope.clone(),
                    requester_extension: request.requester_extension.clone(),
                    identity: request.identity,
                    secret: "changed".into(),
                })
                .await
                .unwrap_err()
                .status_code,
            409
        );
        let mut changed = request.clone();
        changed.target = ExtensionCredentialTarget::Existing {
            account_id: submitted.credential_ref,
        };
        assert_eq!(
            service
                .prepare_manual_token(changed)
                .await
                .unwrap_err()
                .status_code,
            409
        );
        let mut foreign = reference.clone();
        foreign.scope.resource.invocation_id = InvocationId::new();
        assert_eq!(
            service
                .continue_manual_token(foreign)
                .await
                .unwrap_err()
                .status_code,
            403
        );
        let mut canceled = request;
        canceled.identity = ManualTokenSetupIdentity::default();
        service
            .prepare_manual_token(canceled.clone())
            .await
            .unwrap();
        let canceled_reference = ExtensionCredentialReferenceRequest {
            identity: canceled.identity,
            ..reference
        };
        assert_eq!(
            service
                .cancel_manual_token(canceled_reference)
                .await
                .unwrap()
                .status,
            AuthFlowStatus::Canceled
        );
        assert_eq!(shared.flow_records_snapshot().len(), 2);
    }

    #[tokio::test]
    async fn memory_extension_requirement_binding_controls_readiness_and_original_recovery() {
        let shared = Arc::new(InMemoryAuthProductServices::new());
        super::verify_extension_requirement_readiness(
            RebornProductAuthServices::from_shared(shared, Arc::new(NoopDispatcher)),
            test_scope(),
        )
        .await;
    }

    #[tokio::test]
    async fn memory_extension_metadata_expiry_never_accepts_raw_or_erases_completed_proof() {
        let shared = Arc::new(InMemoryAuthProductServices::new());
        super::verify_extension_metadata_expiry(
            RebornProductAuthServices::from_shared(shared, Arc::new(NoopDispatcher)),
            test_scope(),
        )
        .await;
    }

    async fn seed_account(
        shared: &InMemoryAuthProductServices,
        scope: AuthProductScope,
        label: &str,
    ) {
        let handle_label = label.replace(' ', "-");
        shared
            .create_account(NewCredentialAccount {
                scope,
                provider: AuthProviderId::new("notion").expect("provider"),
                label: CredentialAccountLabel::new(label.to_string()).expect("label"),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(
                    SecretHandle::new(format!("{handle_label}-access")).expect("secret"),
                ),
                access_expires_at: None,
                refresh_secret: None,
                scopes: Vec::new(),
            })
            .await
            .expect("seed account");
    }

    fn test_scope() -> AuthProductScope {
        AuthProductScope::new(
            ResourceScope {
                tenant_id: TenantId::new("tenant-alpha").expect("tenant"),
                user_id: UserId::new("user-alpha").expect("user"),
                agent_id: None,
                project_id: None,
                thread_id: None,
                invocation_id: InvocationId::new(),
            },
            AuthSurface::Callback,
        )
    }
}
