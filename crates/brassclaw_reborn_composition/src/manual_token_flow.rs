use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_auth::{
    AuthChallenge, AuthFlowId, AuthFlowKind, AuthFlowManager, AuthFlowRecord, AuthFlowStatus,
    AuthInteractionId, AuthInteractionService, AuthProductError, AuthProductScope,
    InMemoryAuthProductServices, ManualTokenCompletionInput, ManualTokenSetupIdentity,
    ManualTokenSetupRequest, NewAuthFlow, SecretSubmitRequest, SecretSubmitResult,
};
use brassclaw_filesystem::RootFilesystem;
use chrono::Utc;
use secrecy::SecretString;

use crate::product_auth_durable::FilesystemAuthProductServices;

#[async_trait]
#[doc(hidden)]
pub trait RebornManualTokenFlowService: Send + Sync {
    async fn request_manual_token_flow(
        &self,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthChallenge, AuthProductError>;

    /// Create/read the same original setup and its actual retained flow state.
    /// Concrete shared owners must retain both IDs, first expiry and the account
    /// snapshot before acknowledgement. Separate arbitrary lower ports cannot
    /// manufacture this custody by minting a replacement setup.
    async fn request_original_manual_token_flow(
        &self,
        _identity: ManualTokenSetupIdentity,
        _request: ManualTokenSetupRequest,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        Err(AuthProductError::BackendUnavailable)
    }

    /// Recover a genuine existing setup and its original binding/first expiry.
    /// Missing original returns None; partial/orphan/incoherent proof fails.
    /// FS may reconcile its projection from the retained original interaction,
    /// but must not create a new original, use a current account, accept raw data
    /// or dispatch continuation. Arbitrary lower-port composition is insufficient.
    async fn recover_original_manual_token_flow(
        &self,
        _scope: &AuthProductScope,
        _identity: ManualTokenSetupIdentity,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        Err(AuthProductError::BackendUnavailable)
    }

    /// Continue a confirmed original completion using metadata only. This is
    /// separate from accepting any new raw token and must not publish an intent
    /// that lacks an actual original completion or substitute today's account.
    async fn continue_original_manual_token_flow(
        &self,
        _scope: &AuthProductScope,
        _identity: ManualTokenSetupIdentity,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        Err(AuthProductError::BackendUnavailable)
    }

    /// Match the full original pair before any credential publication.
    async fn submit_original_manual_token_flow(
        &self,
        _scope: &AuthProductScope,
        _identity: ManualTokenSetupIdentity,
        _secret: SecretString,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        Err(AuthProductError::BackendUnavailable)
    }

    async fn submit_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        request: SecretSubmitRequest,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError>;

    async fn abandon_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<bool, AuthProductError>;
}

#[derive(Clone)]
pub(crate) struct PortBackedManualTokenFlowService {
    flow_manager: Arc<dyn AuthFlowManager>,
    interaction_service: Arc<dyn AuthInteractionService>,
}

impl PortBackedManualTokenFlowService {
    pub(crate) fn new(
        flow_manager: Arc<dyn AuthFlowManager>,
        interaction_service: Arc<dyn AuthInteractionService>,
    ) -> Self {
        Self {
            flow_manager,
            interaction_service,
        }
    }
}

#[async_trait]
impl RebornManualTokenFlowService for PortBackedManualTokenFlowService {
    async fn request_manual_token_flow(
        &self,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthChallenge, AuthProductError> {
        request_manual_token_flow_with(
            self.flow_manager.as_ref(),
            self.interaction_service.as_ref(),
            request,
        )
        .await
    }

    async fn submit_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        request: SecretSubmitRequest,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        submit_manual_token_flow_with(
            self.flow_manager.as_ref(),
            self.interaction_service.as_ref(),
            scope,
            request,
        )
        .await
    }

    async fn abandon_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        abandon_manual_token_flow_with(
            self.flow_manager.as_ref(),
            self.interaction_service.as_ref(),
            scope,
            interaction_id,
        )
        .await
    }
}

#[async_trait]
impl RebornManualTokenFlowService for InMemoryAuthProductServices {
    async fn request_manual_token_flow(
        &self,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthChallenge, AuthProductError> {
        request_manual_token_flow_with(self, self, request).await
    }

    async fn recover_original_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        self.recover_manual_setup_original_for_tests(scope, identity)
    }

    async fn request_original_manual_token_flow(
        &self,
        identity: ManualTokenSetupIdentity,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.request_manual_setup_original_for_tests(identity, request)
    }
    async fn continue_original_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        self.continue_manual_setup_original_for_tests(scope, identity)
    }

    async fn submit_original_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
        secret: SecretString,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        self.submit_original_manual_token_flow_for_tests(scope, identity, secret)
    }

    async fn submit_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        request: SecretSubmitRequest,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        self.submit_manual_token_flow_for_tests(scope, request)
    }

    async fn abandon_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        abandon_manual_token_flow_with(self, self, scope, interaction_id).await
    }
}

#[async_trait]
impl<F> RebornManualTokenFlowService for FilesystemAuthProductServices<F>
where
    F: RootFilesystem + 'static,
{
    async fn request_manual_token_flow(
        &self,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthChallenge, AuthProductError> {
        request_manual_token_flow_with(self, self, request).await
    }

    async fn submit_original_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
        secret: SecretString,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        let (result, flow) = self
            .submit_manual_owned(
                scope,
                SecretSubmitRequest {
                    interaction_id: identity.interaction_id,
                    secret,
                },
                true,
                Some(identity.flow_id),
            )
            .await?;
        Ok((result, flow.ok_or(AuthProductError::BackendConflict)?))
    }

    async fn submit_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        request: SecretSubmitRequest,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        let (result, flow) = self.submit_manual_owned(scope, request, true, None).await?;
        Ok((result, flow.ok_or(AuthProductError::BackendConflict)?))
    }

    async fn recover_original_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        self.recover_fs_manual_setup_original(scope, identity).await
    }

    async fn request_original_manual_token_flow(
        &self,
        identity: ManualTokenSetupIdentity,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.request_fs_manual_setup_original(identity, request)
            .await
    }

    async fn continue_original_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        self.continue_manual_original_owned(scope, identity).await
    }

    async fn abandon_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        self.abandon_manual_flow_owned(scope, interaction_id).await
    }
}

pub(crate) use brassclaw_auth::domain::{
    build_original_manual_setup_flow, validate_original_manual_setup_flow,
};

pub(crate) async fn request_manual_token_flow_with(
    flow_manager: &dyn AuthFlowManager,
    interaction_service: &dyn AuthInteractionService,
    request: ManualTokenSetupRequest,
) -> Result<AuthChallenge, AuthProductError> {
    if request.expires_at <= Utc::now() {
        return Err(AuthProductError::UnknownOrExpiredFlow);
    }
    let flow_scope = request.scope.clone();
    let flow_provider = request.provider.clone();
    let flow_label = request.label.clone();
    let flow_continuation = request.continuation.clone();
    let flow_update_binding = request.update_binding.clone();
    let flow_requirement_setup = request.requirement_setup.clone();
    let flow_expires_at = request.expires_at;
    let challenge = interaction_service.request_secret_input(request).await?;
    let AuthChallenge::ManualTokenRequired {
        interaction_id,
        provider,
        label,
        expires_at,
    } = &challenge
    else {
        return Err(AuthProductError::InvalidRequest {
            reason: "manual token setup returned an unexpected challenge".to_string(),
        });
    };
    if provider != &flow_provider || label != &flow_label || *expires_at != flow_expires_at {
        return Err(AuthProductError::BackendConflict);
    }
    // Choose this identity before the first possible write. The interaction
    // is already the original durable setup handle; it does not grant access.
    // Existing flows/interaction identities are never rewritten or derived anew.
    let flow_id = AuthFlowId::new();
    let original = NewAuthFlow {
        requirement_setup: flow_requirement_setup,
        id: Some(flow_id),
        scope: flow_scope.clone(),
        kind: AuthFlowKind::IntegrationCredential,
        provider: flow_provider,
        challenge: AuthChallenge::ManualTokenRequired {
            interaction_id: *interaction_id,
            provider: provider.clone(),
            label: label.clone(),
            expires_at: *expires_at,
        },
        continuation: flow_continuation,
        update_binding: flow_update_binding,
        opaque_state_hash: None,
        pkce_verifier_hash: None,
        expires_at: flow_expires_at,
    };
    let recorded = match flow_manager.create_flow(original.clone()).await {
        Ok(recorded) => recorded,
        Err(write_error) => {
            // An error/timeout is not proof that creation failed. Never abandon an
            // interaction whose original flow may have committed, including when
            // the original read is unavailable or its write is still unconfirmed.
            let recorded = flow_manager
                .get_flow(&flow_scope, flow_id)
                .await
                .inspect_err(|read_error| {
                    tracing::warn!(
                        %flow_id,
                        write_error_code = ?write_error.code(),
                        readback_error_code = ?read_error.code(),
                        "manual-token flow creation and original readback are unconfirmed"
                    );
                })?;
            recorded.ok_or(write_error)?
        }
    };
    validate_manual_setup_record(&original, &recorded)?;
    if let Some(expected) = &original.requirement_setup
        && flow_manager
            .requirement_setup(&flow_scope, recorded.id)
            .await?
            .as_ref()
            != Some(expected.as_ref())
    {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(challenge)
}

fn validate_manual_setup_record(
    original: &NewAuthFlow,
    recorded: &AuthFlowRecord,
) -> Result<(), AuthProductError> {
    if original.id != Some(recorded.id)
        || original.scope != recorded.scope
        || original.kind != recorded.kind
        || original.provider != recorded.provider
        || recorded.challenge.as_ref() != Some(&original.challenge)
        || original.continuation != recorded.continuation
        || original.update_binding != recorded.update_binding
        || original.opaque_state_hash != recorded.opaque_state_hash
        || original.pkce_verifier_hash != recorded.pkce_verifier_hash
        || original.expires_at != recorded.expires_at
    {
        return Err(AuthProductError::BackendConflict);
    }
    match recorded.status {
        AuthFlowStatus::AwaitingUser => {}
        AuthFlowStatus::Canceled => return Err(AuthProductError::Canceled),
        AuthFlowStatus::Expired => return Err(AuthProductError::UnknownOrExpiredFlow),
        _ => return Err(AuthProductError::FlowAlreadyTerminal),
    }
    if recorded.error.is_some()
        || recorded.credential_account_id.is_some()
        || recorded.authorization_code_hash.is_some()
        || recorded.continuation_emitted_at.is_some()
    {
        return Err(AuthProductError::BackendConflict);
    }
    if recorded.expires_at <= Utc::now() {
        return Err(AuthProductError::UnknownOrExpiredFlow);
    }
    Ok(())
}

pub(crate) async fn submit_manual_token_flow_with(
    flow_manager: &dyn AuthFlowManager,
    interaction_service: &dyn AuthInteractionService,
    scope: &AuthProductScope,
    request: SecretSubmitRequest,
) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
    let interaction_id = request.interaction_id;
    let result = interaction_service
        .submit_manual_token(scope, request)
        .await?;
    // Completion may already have committed when its acknowledgement is lost.
    // An error cannot authorize cancellation or revocation of that original.
    let completed = flow_manager
        .complete_manual_token(
            scope,
            ManualTokenCompletionInput {
                interaction_id,
                credential_account_id: result.account_id,
            },
        )
        .await?;
    Ok((result, completed))
}

async fn abandon_manual_token_flow_with(
    flow_manager: &dyn AuthFlowManager,
    interaction_service: &dyn AuthInteractionService,
    scope: &AuthProductScope,
    interaction_id: AuthInteractionId,
) -> Result<bool, AuthProductError> {
    let interaction_abandoned = interaction_service
        .abandon_manual_token(scope, interaction_id)
        .await?;
    let flow_abandoned = flow_manager
        .cancel_manual_token(scope, interaction_id)
        .await?
        .is_some_and(|flow| flow.status == AuthFlowStatus::Canceled);
    Ok(interaction_abandoned || flow_abandoned)
}

#[cfg(test)]
#[path = "manual_token_flow_tests.rs"]
mod tests;
