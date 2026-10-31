//! Honest continuation of pre-custody DCR flows. Seals real existing inputs;
//! never generates new PKCE, discovers latest endpoints or registers again.
use super::private_material::DcrLegacyPayload;
use super::*;
use brassclaw_auth::{
    AuthFlowRecord, DcrLegacyOriginal, DcrLegacySelection, DcrLegacyState, DcrMaterialReference,
};
use brassclaw_secrets::SecretCustodyState;

fn parameter(url: &url::Url, name: &str) -> Result<String, AuthProductError> {
    let mut values = url
        .query_pairs()
        .filter(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned());
    let value = values.next().ok_or(AuthProductError::BackendConflict)?;
    if values.next().is_some() {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(value)
}
fn endpoint(raw: &str) -> Result<url::Url, AuthProductError> {
    let parsed = url::Url::parse(raw).map_err(|_| AuthProductError::BackendConflict)?;
    oauth_endpoint_host(raw)?;
    if !parsed.username().is_empty() || parsed.password().is_some() || parsed.fragment().is_some() {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(parsed)
}
fn definition(flow: &AuthFlowRecord) -> Result<DcrLegacyOriginal, AuthProductError> {
    if flow.kind != brassclaw_auth::AuthFlowKind::IntegrationCredential {
        return Err(AuthProductError::BackendConflict);
    }
    let Some(AuthChallenge::OAuthUrl {
        authorization_url, ..
    }) = &flow.challenge
    else {
        return Err(AuthProductError::BackendConflict);
    };
    let url = endpoint(authorization_url.as_str())?;
    let state = parameter(&url, "state")?;
    let decoded = DcrOAuthCallbackState::decode(&state)?;
    if decoded.flow_id() != flow.id
        || decoded.scope().resource != flow.scope.resource
        || decoded.scope().session_id != flow.scope.session_id
        || decoded.provider() != &flow.provider
        || !flow
            .opaque_state_hash
            .as_ref()
            .ok_or(AuthProductError::BackendConflict)?
            .constant_time_eq(&opaque_state_hash(&state)?)
    {
        return Err(AuthProductError::BackendConflict);
    }
    if parameter(&url, "response_type")? != "code"
        || parameter(&url, "scope")? != scope_text(decoded.requested_scopes())
    {
        return Err(AuthProductError::BackendConflict);
    }
    DcrLegacyOriginal::from_flow(
        flow,
        decoded.account_label().clone(),
        decoded.requested_scopes().to_vec(),
    )
}
fn validate_payload(
    original: &DcrLegacyOriginal,
    payload: &DcrLegacyPayload,
) -> Result<(), AuthProductError> {
    let url = endpoint(original.authorization_url.as_str())?;
    let verifier = PkceVerifierSecret::new(payload.pkce.clone())?;
    if !original
        .pkce_verifier_hash
        .constant_time_eq(&pkce_verifier_hash(&verifier)?)
        || parameter(&url, "code_challenge")? != pkce_s256_challenge(&verifier).as_str()
        || parameter(&url, "code_challenge_method")? != "S256"
        || parameter(&url, "client_id")? != payload.client.client_id
        || parameter(&url, "redirect_uri")? != payload.client.redirect_uri
    {
        return Err(AuthProductError::BackendConflict);
    }
    OAuthRedirectUri::new(&payload.client.redirect_uri)?;
    OAuthClientId::new(&payload.client.client_id)?;
    endpoint(&payload.client.token_endpoint)?;
    validate_endpoint_origin(&payload.client.token_endpoint, url.as_str())?;
    Ok(())
}

impl OAuthDcrProvider {
    pub(super) fn legacy_definition(
        &self,
        flow: &AuthFlowRecord,
    ) -> Result<DcrLegacyOriginal, AuthProductError> {
        if flow.provider.as_str() != self.spec.provider_id {
            return Err(AuthProductError::BackendConflict);
        }
        definition(flow)
    }
    pub(super) async fn legacy_flow_definition(
        &self,
        flow: &AuthFlowRecord,
    ) -> Result<DcrLegacyOriginal, AuthProductError> {
        let original = self.legacy_definition(flow)?;
        let modern = flow_secret_handle(&self.spec, flow.id, "preparation-original")?;
        if self
            .secret_store
            .record_reference(&flow.scope.resource, &modern)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .is_some()
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(original)
    }
    async fn read_legacy_selection(
        &self,
        scope: &ResourceScope,
        id: AuthFlowId,
    ) -> Result<Option<DcrLegacySelection>, AuthProductError> {
        let handle = flow_secret_handle(&self.spec, id, "legacy-selection-original")?;
        let Some(metadata) = self
            .secret_store
            .custody(scope, &handle)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
        else {
            if self
                .secret_store
                .record_reference(scope, &handle)
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?
                .is_some()
            {
                return Err(AuthProductError::BackendConflict);
            }
            return Ok(None);
        };
        if metadata.scope != *scope || metadata.handle != handle {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let SecretCustodyState::Original { context } = &metadata.state else {
            return Err(AuthProductError::BackendConflict);
        };
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Context {
            format: String,
            original: DcrLegacyOriginal,
            legacy_pkce: DcrMaterialReference,
            legacy_client: DcrMaterialReference,
        }
        let context: Context = serde_json::from_value(context.clone())
            .map_err(|_| AuthProductError::BackendConflict)?;
        if context.format != "legacy-dcr-selection/1"
            || context.original.flow_id != id
            || context.original.scope.resource != *scope
            || context.original.provider.as_str() != self.spec.provider_id
        {
            return Err(AuthProductError::BackendConflict);
        }
        let selection = DcrLegacySelection {
            original: context.original,
            legacy_pkce: context.legacy_pkce,
            legacy_client: context.legacy_client,
            retained: DcrMaterialReference {
                handle,
                reference: metadata.reference,
            },
        };
        selection.validate()?;
        Ok(Some(selection))
    }
    pub(super) async fn load_legacy_payload(
        &self,
        scope: &ResourceScope,
        id: AuthFlowId,
    ) -> Result<Option<DcrLegacyPayload>, AuthProductError> {
        let Some(selection) = self.read_legacy_selection(scope, id).await? else {
            return Ok(None);
        };
        let target = flow_secret_handle(&self.spec, id, "legacy-selection-read")?;
        let retained = self
            .retain_dcr_output(scope, target, selection.retained)
            .await?;
        let raw = self.load_secret(scope, &retained.handle).await?;
        let payload = DcrLegacyPayload::decode(&raw)?;
        validate_payload(&selection.original, &payload)?;
        Ok(Some(payload))
    }
    pub(super) async fn cleanup_legacy_inputs(
        &self,
        scope: &ResourceScope,
        id: AuthFlowId,
    ) -> Result<bool, AuthProductError> {
        let Some(selection) = self.read_legacy_selection(scope, id).await? else {
            return Ok(false);
        };
        // The existing immutable selection is the complete intent. Authenticate
        // exact secret-owner tombstones on readback after a lost close reply.
        // Keep the sealed capsule for callback/recovery and retained client use.
        for source in [&selection.legacy_pkce, &selection.legacy_client] {
            let expected = brassclaw_secrets::SecretIdentityClosure::Ordinary {
                reference: source.reference.clone(),
                context: selection.context(),
            };
            let closed = self
                .secret_store
                .close_identity(scope.clone(), source.handle.clone(), expected.clone())
                .await;
            let metadata = match closed {
                Ok(Some(metadata)) => metadata,
                Ok(None) | Err(_) => {
                    let metadata = self
                        .secret_store
                        .custody(scope, &source.handle)
                        .await
                        .map_err(|_| AuthProductError::BackendUnavailable)?;
                    let Some(metadata) = metadata else {
                        // No tombstone means no confirmed close. Distinguish
                        // a proven replacement from a still-unresolved write.
                        if self
                            .secret_store
                            .record_reference(scope, &source.handle)
                            .await
                            .map_err(|_| AuthProductError::BackendUnavailable)?
                            .is_some_and(|reference| reference != source.reference)
                        {
                            return Err(AuthProductError::BackendConflict);
                        }
                        return Err(AuthProductError::BackendUnavailable);
                    };
                    metadata
                }
            };
            if metadata.handle != source.handle || !expected.accepts(scope, &metadata) {
                return Err(AuthProductError::BackendConflict);
            }
        }
        Ok(true)
    }
    pub(super) async fn retain_legacy_flow(
        &self,
        manager: &Arc<dyn AuthFlowManager>,
        flow: AuthFlowRecord,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        // A missing/corrupt modern action is never reclassified as old input.
        let original = self.legacy_flow_definition(&flow).await?;
        if brassclaw_auth::is_terminal_status(flow.status) {
            return Ok(flow);
        }
        if let Some(selected) = self
            .read_legacy_selection(&flow.scope.resource, flow.id)
            .await?
        {
            selected.original.verify_flow(&flow)?;
            return manager
                .retain_legacy_dcr(selected)
                .await
                .map(|state| state.flow);
        }
        let pkce = flow_secret_handle(&self.spec, flow.id, "pkce")?;
        let client = flow_secret_handle(&self.spec, flow.id, "client")?;
        let mut sources = Vec::new();
        for handle in [&pkce, &client] {
            if self
                .secret_store
                .custody(&flow.scope.resource, handle)
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?
                .is_some()
            {
                return Err(AuthProductError::BackendConflict);
            }
            let reference = self
                .secret_store
                .record_reference(&flow.scope.resource, handle)
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?
                .ok_or(AuthProductError::BackendConflict)?;
            sources.push(DcrMaterialReference {
                handle: handle.clone(),
                reference,
            });
        }
        let payload = DcrLegacyPayload {
            pkce: self.load_secret(&flow.scope.resource, &pkce).await?,
            client: self.load_material(&flow.scope.resource, &client).await?,
        };
        validate_payload(&original, &payload)?;
        for source in &sources {
            if self
                .secret_store
                .record_reference(&flow.scope.resource, &source.handle)
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?
                .as_ref()
                != Some(&source.reference)
            {
                return Err(AuthProductError::BackendConflict);
            }
        }
        let [legacy_pkce, legacy_client]: [DcrMaterialReference; 2] = sources
            .try_into()
            .map_err(|_| AuthProductError::BackendConflict)?;
        let retained = self
            .retain_dcr_original(
                &flow.scope.resource,
                flow_secret_handle(&self.spec, flow.id, "legacy-selection-original")?,
                payload.encode()?,
                DcrLegacySelection::retention_context(&original, &legacy_pkce, &legacy_client),
            )
            .await?;
        let state: DcrLegacyState = manager
            .retain_legacy_dcr(DcrLegacySelection {
                original,
                legacy_pkce,
                legacy_client,
                retained,
            })
            .await?;
        Ok(state.flow)
    }
}
