use super::paths::interaction_path;
use super::{FilesystemAuthProductServices, domain::validate_manual_token_update_binding};
use async_trait::async_trait;
use brassclaw_auth::{
    AuthChallenge, AuthInteractionId, AuthInteractionService, AuthProductError, CredentialAccount,
    CredentialAccountUpdateBinding, ManualTokenSetupRequest, SecretSubmitRequest,
    SecretSubmitResult, Timestamp,
};
use brassclaw_filesystem::{CasExpectation, RootFilesystem};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};

#[async_trait]
impl<F> AuthInteractionService for FilesystemAuthProductServices<F>
where
    F: RootFilesystem + 'static,
{
    async fn request_secret_input(
        &self,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthChallenge, AuthProductError> {
        if let Some(setup) = &request.requirement_setup {
            setup.verify_manual(&request.scope, &request.provider)?;
        }
        let update_account_snapshot = if let Some(binding) = &request.update_binding {
            let account = self
                .read_account(&request.scope, binding.account_id)
                .await?
                .map(|(account, _)| account)
                .ok_or(AuthProductError::CredentialMissing)?;
            validate_manual_token_update_binding(&account, &request, binding)?;
            Some(account)
        } else {
            None
        };
        let interaction = StoredManualTokenInteraction {
            requirement_setup: request.requirement_setup,
            id: AuthInteractionId::new(),
            scope: request.scope,
            provider: request.provider.clone(),
            label: request.label.clone(),
            continuation: request.continuation,
            update_binding: request.update_binding,
            update_account_snapshot,
            publication: None,
            canceled_at: None,
            setup_flow_id: None,
            setup_created_at: None,
            expires_at: request.expires_at,
            consumed_at: None,
        };
        self.write_record(
            &interaction.scope.resource,
            &interaction_path(&interaction.scope, interaction.id),
            &interaction,
            CasExpectation::Absent,
        )
        .await?;
        Ok(AuthChallenge::ManualTokenRequired {
            interaction_id: interaction.id,
            provider: interaction.provider,
            label: interaction.label,
            expires_at: interaction.expires_at,
        })
    }

    async fn submit_manual_token(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        request: SecretSubmitRequest,
    ) -> Result<SecretSubmitResult, AuthProductError> {
        self.submit_manual_owned(scope, request, false, None)
            .await
            .map(|(result, _)| result)
    }
    async fn abandon_manual_token(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        id: AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        self.abandon_manual_interaction_owned(scope, id).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct StoredManualTokenInteraction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) requirement_setup: Option<Box<brassclaw_auth::CredentialRequirementSetup>>,
    pub(super) id: AuthInteractionId,
    pub(super) scope: brassclaw_auth::AuthProductScope,
    pub(super) provider: brassclaw_auth::AuthProviderId,
    pub(super) label: brassclaw_auth::CredentialAccountLabel,
    pub(super) continuation: brassclaw_auth::AuthContinuationRef,
    pub(super) update_binding: Option<CredentialAccountUpdateBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) update_account_snapshot: Option<CredentialAccount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) publication: Option<super::manual::ManualDecision>,
    pub(super) expires_at: Timestamp,
    pub(super) consumed_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) canceled_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) setup_flow_id: Option<brassclaw_auth::AuthFlowId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) setup_created_at: Option<Timestamp>,
}

pub(super) fn validate_secret(request: &SecretSubmitRequest) -> Result<(), AuthProductError> {
    let exposed = request.secret.expose_secret();
    if exposed.trim().is_empty() {
        return Err(AuthProductError::InvalidRequest {
            reason: "secret value must not be empty".to_string(),
        });
    }
    if exposed.chars().any(|c| c == '\0' || c.is_control()) {
        return Err(AuthProductError::InvalidRequest {
            reason: "secret value must not contain NUL/control characters".to_string(),
        });
    }
    Ok(())
}
