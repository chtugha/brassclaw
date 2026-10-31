//! Existing provider's original DCR preparation/dispatch/material path.
//! Recovery only reconciles this original; it never creates a second POST.
use super::private_material::{DcrPreparationRecord, DcrResponseRecord};
use super::*;
use brassclaw_auth::{
    DcrChallengePublication, DcrMaterialReference, DcrRegistrationAdmission,
    DcrRegistrationChallenge, DcrRegistrationCompletion, DcrRegistrationOriginal,
    DcrRegistrationOutcome, DcrRegistrationState,
};
use brassclaw_secrets::{SecretCustodyMetadata, SecretCustodySource, SecretCustodyState};

#[derive(Clone)]
pub(super) struct DcrOriginalStart {
    /// Complete usage and previous selection captured before the challenge.
    pub(super) requirement_setup: Option<Box<brassclaw_auth::CredentialRequirementSetup>>,
    pub(super) scope: AuthProductScope,
    pub(super) flow_id: AuthFlowId,
    pub(super) account_label: CredentialAccountLabel,
    pub(super) provider_scopes: Vec<ProviderScope>,
    pub(super) continuation: AuthContinuationRef,
    pub(super) update_binding: Option<CredentialAccountUpdateBinding>,
    pub(super) expires_at: brassclaw_auth::Timestamp,
}

impl DcrOriginalStart {
    fn verify(
        &self,
        original: &DcrRegistrationOriginal,
        provider: &str,
    ) -> Result<(), AuthProductError> {
        if self.scope != original.scope || self.flow_id != original.flow_id {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if original.provider.as_str() != provider
            || original.account_label != self.account_label
            || original.requested_scopes != self.provider_scopes
            || original.continuation != self.continuation
            || original.update_binding != self.update_binding
            || original.expires_at != self.expires_at
            || original.requirement_setup != self.requirement_setup
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }
}

fn selected(metadata: SecretCustodyMetadata) -> DcrMaterialReference {
    DcrMaterialReference {
        handle: metadata.handle,
        reference: metadata.reference,
    }
}

fn registration_body(
    original: &DcrRegistrationOriginal,
    preparation: &DcrPreparationRecord,
) -> Result<Vec<u8>, AuthProductError> {
    let verifier = PkceVerifierSecret::new(preparation.pkce.clone())?;
    let state_value = OAuthState::new(preparation.state.expose_secret())?;
    if !original
        .pkce_verifier_hash
        .constant_time_eq(&pkce_verifier_hash(&verifier)?)
        || !original
            .opaque_state_hash
            .constant_time_eq(&opaque_state_hash(state_value.as_str())?)
    {
        return Err(AuthProductError::BackendConflict);
    }
    let state = DcrOAuthCallbackState::decode(state_value.as_str())?;
    if state.flow_id() != original.flow_id
        // The wire state rehydrates on the Callback surface. Keep the exact
        // owner/invocation/session identity; the initiating UI surface remains
        // pinned in the encrypted original instead of being compared to Callback.
        || state.scope().resource != original.scope.resource
        || state.scope().session_id != original.scope.session_id
        || state.provider() != &original.provider
        || state.account_label() != &original.account_label
        || state.requested_scopes() != original.requested_scopes.as_slice()
    {
        return Err(AuthProductError::BackendConflict);
    }
    OAuthRedirectUri::new(&preparation.redirect_uri)?;
    OAuthAuthorizationEndpoint::new(&preparation.authorization_endpoint)?;
    validate_endpoint_origin(
        &preparation.token_endpoint,
        &preparation.authorization_endpoint,
    )?;
    validate_endpoint_origin(
        &preparation.registration_endpoint,
        &preparation.authorization_endpoint,
    )?;
    if preparation.client_name.trim().is_empty() {
        return Err(AuthProductError::MalformedConfig);
    }
    let request = DcrRegistrationRequest {
        client_name: &preparation.client_name,
        redirect_uris: vec![preparation.redirect_uri.as_str()],
        grant_types: vec!["authorization_code", "refresh_token"],
        response_types: vec!["code"],
        token_endpoint_auth_method: "none",
    };
    serde_json::to_vec(&request).map_err(|_| AuthProductError::BackendUnavailable)
}

impl OAuthDcrProvider {
    /// First consult the original flow and encrypted preparation. Metadata
    /// discovery and fresh PKCE generation only happen when neither exists.
    pub(super) async fn start_original_registration(
        &self,
        manager: &Arc<dyn AuthFlowManager>,
        request: DcrOriginalStart,
    ) -> Result<brassclaw_auth::AuthFlowRecord, AuthProductError> {
        if let Some(setup) = &request.requirement_setup {
            setup.verify_oauth(
                &request.scope,
                &AuthProviderId::new(self.spec.provider_id)?,
                &request.provider_scopes,
            )?;
        }

        let state = if let Some(existing) = manager
            .read_dcr_registration(&request.scope, request.flow_id)
            .await?
        {
            request.verify(&existing.action.original, self.spec.provider_id)?;
            existing
        } else {
            if let Some(flow) = manager.get_flow(&request.scope, request.flow_id).await? {
                let original = self.legacy_definition(&flow)?;
                if request.requirement_setup.is_some()
                    || original.account_label != request.account_label
                    || original.requested_scopes != request.provider_scopes
                    || original.expires_at != request.expires_at
                    || original.continuation != request.continuation
                    || original.update_binding != request.update_binding
                {
                    return Err(AuthProductError::BackendConflict);
                }
                return self.retain_legacy_flow(manager, flow).await;
            }
            let original = match self.read_preparation_original(&request).await? {
                Some(original) => original,
                None => {
                    if request.provider_scopes != self.scopes {
                        return Err(AuthProductError::BackendUnavailable);
                    }
                    if request.expires_at <= Utc::now() {
                        return Err(AuthProductError::UnknownOrExpiredFlow);
                    }
                    match self.prepare_registration_original(&request).await {
                        Ok(original) => original,
                        Err(error) => match self.read_preparation_original(&request).await? {
                            Some(original) => original,
                            None => return Err(error),
                        },
                    }
                }
            };
            manager.prepare_dcr_registration(original).await?
        };
        self.continue_original_registration(manager, state).await
    }

    async fn read_preparation_original(
        &self,
        request: &DcrOriginalStart,
    ) -> Result<Option<DcrRegistrationOriginal>, AuthProductError> {
        let original = self
            .retained_preparation_original(&request.scope, request.flow_id)
            .await?;
        if let Some(original) = &original {
            request.verify(original, self.spec.provider_id)?;
        }
        Ok(original)
    }

    pub(super) async fn retained_preparation_original(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<DcrRegistrationOriginal>, AuthProductError> {
        let handle = flow_secret_handle(&self.spec, flow_id, "preparation-original")?;
        let Some(metadata) = self
            .secret_store
            .custody(&scope.resource, &handle)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
        else {
            return Ok(None);
        };
        if metadata.scope != scope.resource || metadata.handle != handle {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let SecretCustodyState::Original { context } = &metadata.state else {
            return Err(AuthProductError::BackendConflict);
        };
        if !matches!(
            context["format"].as_str(),
            Some("dcr-registration-preparation/1" | "dcr-registration-preparation/2")
        ) {
            return Err(AuthProductError::BackendConflict);
        }
        let mut value = context.clone();
        let object = value
            .as_object_mut()
            .ok_or(AuthProductError::BackendConflict)?;
        object.remove("format");
        object.insert(
            "preparation".into(),
            serde_json::to_value(selected(metadata.clone()))
                .map_err(|_| AuthProductError::BackendUnavailable)?,
        );
        let original: DcrRegistrationOriginal =
            serde_json::from_value(value).map_err(|_| AuthProductError::BackendConflict)?;
        if original.preparation_context() != *context {
            return Err(AuthProductError::BackendConflict);
        }
        if original.scope != *scope || original.flow_id != flow_id {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if original.provider.as_str() != self.spec.provider_id {
            return Err(AuthProductError::BackendConflict);
        }
        original.verify(&original)?;
        if let Some(setup) = &original.requirement_setup {
            setup.verify_oauth(scope, &original.provider, &original.requested_scopes)?;
        }
        Ok(Some(original))
    }

    async fn prepare_registration_original(
        &self,
        request: &DcrOriginalStart,
    ) -> Result<DcrRegistrationOriginal, AuthProductError> {
        if request.expires_at <= chrono::Utc::now() {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }

        let metadata = self
            .discover_authorization_server(&request.scope.resource)
            .await?;
        let verifier = PkceVerifierSecret::new(SecretString::from(
            brassclaw_common::pkce::generate_code_verifier(),
        ))?;
        let provider = AuthProviderId::new(self.spec.provider_id)?;
        let state = DcrOAuthCallbackState::new(
            request.flow_id,
            request.scope.clone(),
            provider.clone(),
            request.account_label.clone(),
            request.provider_scopes.clone(),
        )
        .encode()?;
        let redirect =
            self.callback_redirect_uri(&request.scope, request.flow_id, &request.account_label)?;
        let handle = flow_secret_handle(&self.spec, request.flow_id, "preparation-original")?;
        let mut original = DcrRegistrationOriginal {
            requirement_setup: request.requirement_setup.clone(),
            scope: request.scope.clone(),
            flow_id: request.flow_id,
            provider,
            account_label: request.account_label.clone(),
            requested_scopes: request.provider_scopes.clone(),
            continuation: request.continuation.clone(),
            update_binding: request.update_binding.clone(),
            opaque_state_hash: opaque_state_hash(state.as_str())?,
            pkce_verifier_hash: pkce_verifier_hash(&verifier)?,
            expires_at: request.expires_at,
            preparation: DcrMaterialReference {
                handle: handle.clone(),
                reference: "not-yet-written".into(),
            },
        };
        // Raw fields exist only in this encrypted payload, never Debug/errors,
        // safe Auth records, route DTOs or browser persistence.
        let raw = DcrPreparationRecord {
            format: "dcr-private-preparation/1".into(),
            pkce: SecretString::from(verifier.expose_secret().to_owned()),
            state: SecretString::from(state.as_str().to_owned()),
            redirect_uri: redirect.as_str().to_owned(),
            authorization_endpoint: metadata.authorization_endpoint,
            token_endpoint: metadata.token_endpoint,
            registration_endpoint: metadata.registration_endpoint,
            client_name: self.client_name.clone(),
            resource: self.spec.resource.map(str::to_owned),
        }
        .encode()?;
        original.preparation = self
            .retain_dcr_original(
                &request.scope.resource,
                handle,
                raw,
                original.preparation_context(),
            )
            .await?;
        Ok(original)
    }

    pub(super) async fn retain_dcr_original(
        &self,
        scope: &ResourceScope,
        handle: SecretHandle,
        material: SecretString,
        context: serde_json::Value,
    ) -> Result<DcrMaterialReference, AuthProductError> {
        let result = self
            .secret_store
            .put_original(
                scope.clone(),
                handle.clone(),
                material.clone(),
                context.clone(),
            )
            .await;
        let metadata = match result {
            Ok(Some(metadata)) => metadata,
            Ok(None) | Err(_) => {
                let metadata = self
                    .secret_store
                    .custody(scope, &handle)
                    .await
                    .map_err(|_| AuthProductError::BackendUnavailable)?
                    .ok_or(AuthProductError::BackendUnavailable)?;
                if !self
                    .secret_store
                    .matches(scope, &handle, &material)
                    .await
                    .map_err(|_| AuthProductError::BackendUnavailable)?
                {
                    return Err(AuthProductError::BackendConflict);
                }
                metadata
            }
        };
        if metadata.scope != *scope
            || metadata.handle != handle
            || metadata.state != (SecretCustodyState::Original { context })
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(selected(metadata))
    }

    pub(super) async fn retain_dcr_output(
        &self,
        scope: &ResourceScope,
        target: SecretHandle,
        original: DcrMaterialReference,
    ) -> Result<DcrMaterialReference, AuthProductError> {
        let source = SecretCustodySource {
            scope: scope.clone(),
            handle: original.handle,
            reference: original.reference,
        };
        let written = self
            .secret_store
            .put_retained_from_original(scope.clone(), target.clone(), source.clone())
            .await;
        let metadata = match written {
            Ok(Some(metadata)) => metadata,
            Ok(None) | Err(_) => self
                .secret_store
                .custody(scope, &target)
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?
                .ok_or(AuthProductError::BackendUnavailable)?,
        };
        if metadata.scope != *scope
            || metadata.handle != target
            || metadata.state != (SecretCustodyState::Retained { source })
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(selected(metadata))
    }

    pub(super) async fn read_dcr_material(
        &self,
        original: &DcrRegistrationOriginal,
        source: DcrMaterialReference,
        usage: &'static str,
    ) -> Result<SecretString, AuthProductError> {
        let target = flow_secret_handle(&self.spec, original.flow_id, usage)?;
        let retained = self
            .retain_dcr_output(&original.scope.resource, target, source)
            .await?;
        self.load_secret(&original.scope.resource, &retained.handle)
            .await
    }

    pub(super) async fn continue_original_registration(
        &self,
        manager: &Arc<dyn AuthFlowManager>,
        mut state: DcrRegistrationState,
    ) -> Result<brassclaw_auth::AuthFlowRecord, AuthProductError> {
        let original = state.action.original.clone();
        if brassclaw_auth::is_terminal_status(state.flow.status)
            || state.flow.expires_at <= Utc::now()
        {
            return self.cleanup_original_registration(manager, state).await;
        }
        if state.action.publication != DcrChallengePublication::Pending {
            return Ok(state.flow);
        }
        let preparation = self
            .read_dcr_material(&original, original.preparation.clone(), "preparation-read")
            .await?;
        let preparation = DcrPreparationRecord::decode(&preparation)?;
        // Validate every local field before consuming the sole dispatch.
        let body = registration_body(&original, &preparation)?;
        if matches!(state.action.outcome, DcrRegistrationOutcome::Prepared) {
            // Missing policy must not consume the one registration dispatch.
            let http_limits = self.capture_http_limits().await?;
            let admission = manager.claim_dcr_registration(&original).await?;
            let dispatched = match admission {
                DcrRegistrationAdmission::Dispatch(dispatched) => dispatched,
                DcrRegistrationAdmission::Original(existing) => {
                    return self
                        .finish_original_registration(manager, existing, &preparation)
                        .await;
                }
            };
            // A durable claim already exists before this sole HTTP attempt.
            match self
                .execute_request(
                    original.scope.resource.clone(),
                    NetworkMethod::Post,
                    &preparation.registration_endpoint,
                    body,
                    http_limits,
                    None,
                )
                .await
            {
                Ok(response) => {
                    let status = response.status;
                    let context = dispatched.action.response_context(status)?;
                    let retained = self
                        .retain_dcr_original(
                            &original.scope.resource,
                            flow_secret_handle(
                                &self.spec,
                                original.flow_id,
                                "registration-response",
                            )?,
                            DcrResponseRecord::capture(response).encode()?,
                            context,
                        )
                        .await?;
                    let submitted_at = dispatched
                        .action
                        .submitted_at()
                        .ok_or(AuthProductError::BackendConflict)?;
                    state = manager
                        .record_dcr_registration(
                            &original.scope,
                            DcrRegistrationCompletion {
                                dispatched: dispatched.action,
                                outcome: DcrRegistrationOutcome::Returned {
                                    submitted_at,
                                    status,
                                    response: retained,
                                },
                            },
                        )
                        .await?;
                }
                Err(error) => {
                    let submitted_at = dispatched
                        .action
                        .submitted_at()
                        .ok_or(AuthProductError::BackendConflict)?;
                    manager
                        .record_dcr_registration(
                            &original.scope,
                            DcrRegistrationCompletion {
                                dispatched: dispatched.action,
                                outcome: DcrRegistrationOutcome::Unknown {
                                    submitted_at,
                                    error: error.code(),
                                },
                            },
                        )
                        .await?;
                    return Err(error);
                }
            }
        }
        self.finish_original_registration(manager, state, &preparation)
            .await
    }

    pub(super) async fn reconcile_original_response(
        &self,
        manager: &Arc<dyn AuthFlowManager>,
        mut state: DcrRegistrationState,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        let original = state.action.original.clone();
        if matches!(
            state.action.outcome,
            DcrRegistrationOutcome::Submitted { .. }
        ) {
            // A lost response-record acknowledgement can be repaired from the
            // exact encrypted return. Absence never permits another HTTP POST.
            let handle = flow_secret_handle(&self.spec, original.flow_id, "registration-response")?;
            let Some(metadata) = self
                .secret_store
                .custody(&original.scope.resource, &handle)
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?
            else {
                return Ok(state);
            };
            let SecretCustodyState::Original { context } = &metadata.state else {
                return Err(AuthProductError::BackendConflict);
            };
            let status = context["status"]
                .as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .ok_or(AuthProductError::BackendConflict)?;
            if metadata.scope != original.scope.resource
                || metadata.handle != handle
                || *context != state.action.response_context(status)?
            {
                return Err(AuthProductError::BackendConflict);
            }
            let submitted_at = state
                .action
                .submitted_at()
                .ok_or(AuthProductError::BackendConflict)?;
            state = manager
                .record_dcr_registration(
                    &original.scope,
                    DcrRegistrationCompletion {
                        dispatched: state.action,
                        outcome: DcrRegistrationOutcome::Returned {
                            submitted_at,
                            status,
                            response: selected(metadata),
                        },
                    },
                )
                .await?;
        }
        Ok(state)
    }

    async fn finish_original_registration(
        &self,
        manager: &Arc<dyn AuthFlowManager>,
        mut state: DcrRegistrationState,
        preparation: &DcrPreparationRecord,
    ) -> Result<brassclaw_auth::AuthFlowRecord, AuthProductError> {
        let original = state.action.original.clone();
        if brassclaw_auth::is_terminal_status(state.flow.status)
            || state.flow.expires_at <= Utc::now()
        {
            return self.cleanup_original_registration(manager, state).await;
        }
        if state.action.publication != DcrChallengePublication::Pending {
            return Ok(state.flow);
        }
        state = self.reconcile_original_response(manager, state).await?;
        let DcrRegistrationOutcome::Returned {
            status, response, ..
        } = &state.action.outcome
        else {
            return Err(AuthProductError::BackendUnavailable);
        };
        if !(200..300).contains(status) {
            return Err(AuthProductError::BackendUnavailable);
        }
        let response = self
            .read_dcr_material(&original, response.clone(), "registration-response-read")
            .await?;
        let response = DcrResponseRecord::decode(&response)?;
        if response.status != *status {
            return Err(AuthProductError::BackendConflict);
        }
        let body = response.body()?;
        let response: DcrRegistrationResponse = serde_json::from_slice(body.expose_secret())
            .map_err(|_| AuthProductError::BackendUnavailable)?;
        let client_id = OAuthClientId::new(response.client_id)?;
        let pkce = PkceVerifierSecret::new(preparation.pkce.clone())?;
        let state_value = OAuthState::new(preparation.state.expose_secret())?;
        if !original
            .pkce_verifier_hash
            .constant_time_eq(&pkce_verifier_hash(&pkce)?)
            || !original
                .opaque_state_hash
                .constant_time_eq(&opaque_state_hash(state_value.as_str())?)
        {
            return Err(AuthProductError::BackendConflict);
        }
        let endpoint = OAuthAuthorizationEndpoint::new(&preparation.authorization_endpoint)?;
        let redirect = OAuthRedirectUri::new(&preparation.redirect_uri)?;
        let extra = preparation
            .resource
            .as_deref()
            .map(|resource| OAuthExtraParam::new("resource", resource))
            .transpose()?
            .into_iter()
            .collect::<Vec<_>>();
        let authorization_url = build_authorization_url(OAuthAuthorizeUrlRequest {
            authorization_endpoint: &endpoint,
            client_id: &client_id,
            redirect_uri: &redirect,
            state: &state_value,
            code_challenge: &pkce_s256_challenge(&pkce),
            scopes: &original.requested_scopes,
            extra_params: &extra,
        })?;
        let client = StoredDcrClientMaterial {
            client_id: client_id.as_str().into(),
            client_secret: None,
            redirect_uri: redirect.as_str().into(),
            token_endpoint: preparation.token_endpoint.clone(),
        };
        let mut outputs = Vec::new();
        for (usage, raw, source_kind) in [
            ("pkce", preparation.pkce.clone(), "pkce-original"),
            (
                "client",
                SecretString::from(
                    serde_json::to_string(&client)
                        .map_err(|_| AuthProductError::BackendUnavailable)?,
                ),
                "client-original",
            ),
        ] {
            let retained = self
                .retain_dcr_original(
                    &original.scope.resource,
                    flow_secret_handle(&self.spec, original.flow_id, source_kind)?,
                    raw,
                    state.action.output_context(usage, &authorization_url)?,
                )
                .await?;
            outputs.push(
                self.retain_dcr_output(
                    &original.scope.resource,
                    flow_secret_handle(&self.spec, original.flow_id, usage)?,
                    retained,
                )
                .await?,
            );
        }
        let [pkce, client]: [DcrMaterialReference; 2] = outputs
            .try_into()
            .map_err(|_| AuthProductError::BackendConflict)?;
        let published = manager
            .publish_dcr_challenge(
                &original,
                DcrRegistrationChallenge {
                    authorization_url,
                    pkce,
                    client,
                },
            )
            .await?;
        if brassclaw_auth::is_terminal_status(published.flow.status)
            || published.flow.expires_at <= Utc::now()
        {
            self.cleanup_original_registration(manager, published).await
        } else {
            Ok(published.flow)
        }
    }
}
