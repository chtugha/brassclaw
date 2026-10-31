//! Google setup/callback custody through existing Auth and Secret owners.
//! Product routes do not retain private PKCE bytes or select provider clients.
use super::*;
use crate::oauth_gate::GoogleSetupRequest;
use brassclaw_auth::{GoogleOAuthRouteConfig, OAuthSetupOriginal};

fn verify_lookup(
    original: &OAuthSetupOriginal,
    scope: &AuthProductScope,
    flow_id: AuthFlowId,
    label: &CredentialAccountLabel,
    scopes: &[ProviderScope],
    expires_at: Timestamp,
) -> Result<(), AuthProductError> {
    original.validate()?;
    let definition = &original.definition;
    if definition.scope != *scope || definition.flow_id != flow_id {
        return Err(AuthProductError::CrossScopeDenied);
    }
    if definition.provider.as_str() != brassclaw_auth::GOOGLE_PROVIDER_ID
        || definition.continuation != AuthContinuationRef::SetupOnly
        || definition.account_label != *label
        || definition.requested_scopes != scopes
        || definition.expires_at != expires_at
    {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(())
}

impl RebornProductAuthServices {
    /// Read/reconstruct exactly the retained setup before any current manifest,
    /// account or provider configuration lookup. Retirement remains terminal.
    pub(crate) async fn recover_google_setup_original(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
        label: &CredentialAccountLabel,
        scopes: &[ProviderScope],
        expires_at: Timestamp,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        if let Some(stored) = self.flow_manager.read_oauth_setup(scope, flow_id).await? {
            verify_lookup(&stored.original, scope, flow_id, label, scopes, expires_at)?;
            return Ok(Some(stored.flow));
        }
        if let Some(registry) = &self.oauth_gate_registry {
            let provider = registry.google_provider()?;
            if let Some(original) = provider.retained_setup_original(scope, flow_id).await? {
                verify_lookup(&original, scope, flow_id, label, scopes, expires_at)?;
                let restored = provider
                    .restore_setup_original(&self.flow_manager, original, None)
                    .await?;
                return Ok(Some(restored.flow));
            }
        }
        // Historical metadata remains readable, but no private original or
        // current-client equivalence is fabricated for those old records.
        self.recover_google_setup_original_legacy(scope, flow_id, label, scopes, expires_at)
            .await
    }

    /// The existing host client owner supplies complete private material. The
    /// route configuration only checks that its public target agrees at start.
    pub(crate) async fn start_captured_google_setup(
        &self,
        mut request: GoogleSetupRequest,
        route_target: &GoogleOAuthRouteConfig,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        if !matches!(
            request.deadline,
            crate::oauth_gate::GoogleSetupDeadline::Exact(_)
        ) || request.continuation != AuthContinuationRef::SetupOnly
        {
            return Err(AuthProductError::BackendConflict);
        }
        let provider = self
            .oauth_gate_registry
            .as_ref()
            .ok_or(AuthProductError::BackendUnavailable)?
            .google_provider()?;
        if let Some(original) = provider
            .retained_setup_original(&request.scope, request.flow_id)
            .await?
        {
            // Retry never fetches today's account, even after it was changed or
            // deleted. All other caller fields still undergo exact comparison.
            request.update_account_snapshot = original.definition.update_account_snapshot;
        } else {
            if self
                .flow_manager
                .get_flow(&request.scope, request.flow_id)
                .await?
                .is_some()
            {
                return Err(AuthProductError::BackendConflict);
            }
            provider.verify_route_configuration(route_target)?;
            if let Some(binding) = &request.update_binding {
                let mut lookup = brassclaw_auth::CredentialAccountLookupRequest::new(
                    request.scope.clone(),
                    binding.account_id,
                );
                if let Some(setup) = &request.requirement_setup {
                    lookup =
                        lookup.for_extension(setup.requirement().requester_extension().clone());
                }
                request.update_account_snapshot = Some(
                    self.credential_account_service
                        .get_account(lookup)
                        .await?
                        .ok_or(AuthProductError::CredentialMissing)?,
                );
            }
        }
        let original = provider.prepare_setup_original(request).await?;
        let state = provider
            .restore_setup_original(&self.flow_manager, original, None)
            .await?;
        Ok(state.flow)
    }

    pub(crate) async fn cancel_google_setup_original(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
        label: &CredentialAccountLabel,
        scopes: &[ProviderScope],
        expires_at: Timestamp,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let flow = self
            .cancel_google_setup_flow(scope, flow_id, label, scopes, expires_at)
            .await?;
        self.retire_google_original_if_terminal(&flow).await?;
        Ok(flow)
    }

    /// Runs only after the Auth owner's durable terminal readback. Unknown
    /// callback results retain their source; retirement cannot authorize replay.
    pub(super) async fn retire_google_original_if_terminal(
        &self,
        flow: &AuthFlowRecord,
    ) -> Result<(), AuthProductError> {
        if flow.provider.as_str() != brassclaw_auth::GOOGLE_PROVIDER_ID
            || !matches!(
                flow.status,
                AuthFlowStatus::Completed | AuthFlowStatus::Canceled | AuthFlowStatus::Expired
            )
        {
            return Ok(());
        }
        let Some(state) = self
            .flow_manager
            .read_oauth_setup(&flow.scope, flow.id)
            .await?
        else {
            return Ok(()); // Old custody has no new private original to retire.
        };
        state.original.definition.verify_flow(flow)?;
        self.oauth_gate_registry
            .as_ref()
            .ok_or(AuthProductError::BackendUnavailable)?
            .google_provider()?
            .retire_setup_material(&flow.scope.resource, flow.id)
            .await
    }
}
