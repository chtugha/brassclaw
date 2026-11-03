use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_auth::{
    AuthFlowId, AuthProductError, AuthProviderClient, CredentialAccountId, OAuthClientId,
    OAuthProviderCallbackRequest, OAuthProviderExchange, OAuthProviderExchangeContext,
    OAuthProviderRefresh, OAuthProviderRefreshRequest, OAuthRedirectUri, OAuthTokenResponse,
    ProviderScope, validate_provider_callback_request,
};
use brassclaw_capabilities::{
    CapabilityObligationHandler, CapabilityObligationPhase, CapabilityObligationRequest,
};
use brassclaw_host_api::{
    CapabilityId, CapabilitySet, CorrelationId, ExtensionId, MountView, NetworkMethod,
    NetworkPolicy, NetworkScheme, NetworkTargetPattern, Obligation, ResourceEstimate,
    ResourceScope, RuntimeCredentialInjection, RuntimeHttpEgress, RuntimeHttpEgressRequest,
    RuntimeKind, SecretHandle, TrustClass,
};
use brassclaw_secrets::SecretStore;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

/// Host input for the existing original owner, never a browser expiry override.
/// A gate captures its first expiry at preparation; repeats adopt that original.
#[derive(Clone, Copy)]
pub(crate) enum OAuthSetupDeadline {
    Exact(brassclaw_auth::Timestamp),
    GateDefault,
}

impl OAuthSetupDeadline {
    pub(crate) fn validate_continuation(
        self,
        continuation: &brassclaw_auth::AuthContinuationRef,
    ) -> Result<(), AuthProductError> {
        if matches!(self, Self::GateDefault)
            && !matches!(
                continuation,
                brassclaw_auth::AuthContinuationRef::TurnGateResume { .. }
            )
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }

    pub(crate) fn matches_expiry(self, original: brassclaw_auth::Timestamp) -> bool {
        match self {
            Self::Exact(expiry) => expiry == original,
            Self::GateDefault => true,
        }
    }

    pub(crate) async fn resolve_new(
        self,
        source: Option<&Arc<dyn brassclaw_host_runtime::HostPrimitiveSettingsSource>>,
    ) -> Result<brassclaw_auth::Timestamp, AuthProductError> {
        match self {
            Self::Exact(expiry) => Ok(expiry),
            Self::GateDefault => {
                let limits = acknowledged_oauth_limits(source).await?;
                chrono::Utc::now()
                    .checked_add_signed(chrono::Duration::seconds(i64::from(
                        limits.turn_gate_oauth_setup_ttl_secs,
                    )))
                    .ok_or(AuthProductError::BackendUnavailable)
            }
        }
    }
}

/// Both OAuth protocol clients use the existing acknowledged host policy.
/// A configured unavailable owner never falls back to local canonical defaults.
pub(crate) async fn acknowledged_oauth_limits(
    source: Option<&Arc<dyn brassclaw_host_runtime::HostPrimitiveSettingsSource>>,
) -> Result<brassclaw_host_api::MontyExecutionLimits, AuthProductError> {
    let limits = match source {
        Some(source) => {
            let snapshot = source
                .snapshot()
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?;
            if snapshot.revision == 0 {
                return Err(AuthProductError::BackendUnavailable);
            }
            snapshot.limits
        }
        None => brassclaw_host_api::MontyExecutionLimits::default(),
    };
    limits
        .validate()
        .map_err(|_| AuthProductError::BackendUnavailable)?;
    Ok(limits)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExchangeScopePolicy {
    RequireProviderScope,
    FallbackToRequested,
}

#[derive(Debug, Clone)]
pub(crate) struct HostOAuthProviderSpec {
    pub(crate) provider_id: &'static str,
    pub(crate) capability_id: &'static str,
    pub(crate) token_endpoint: &'static str,
    pub(crate) secret_handle_prefix: &'static str,
    pub(crate) resource: Option<&'static str>,
    pub(crate) exchange_scope_policy: ExchangeScopePolicy,
}

#[derive(Clone, Debug)]
pub(crate) struct OAuthClientMaterial {
    pub(crate) client_id: OAuthClientId,
    pub(crate) client_secret: Option<SecretString>,
    pub(crate) redirect_uri: OAuthRedirectUri,
    pub(crate) token_endpoint: String,
}

#[async_trait]
pub(crate) trait OAuthClientMaterialSource: Send + Sync + fmt::Debug {
    async fn exchange_material(
        &self,
        scope: &ResourceScope,
        flow_id: AuthFlowId,
    ) -> Result<OAuthClientMaterial, AuthProductError>;

    async fn refresh_material(
        &self,
        scope: &ResourceScope,
        refresh_secret: &SecretHandle,
    ) -> Result<OAuthClientMaterial, AuthProductError>;

    async fn bind_refresh_material(
        &self,
        scope: &ResourceScope,
        flow_id: AuthFlowId,
        refresh_secret: &SecretHandle,
    ) -> Result<(), AuthProductError>;

    /// Preserve the exact client registration when a provider rotates its
    /// refresh secret. Static clients require no per-secret association.
    async fn bind_rotated_refresh_material(
        &self,
        scope: &ResourceScope,
        previous: &SecretHandle,
        replacement: &SecretHandle,
    ) -> Result<(), AuthProductError>;

    async fn cleanup_refresh_material(
        &self,
        scope: &ResourceScope,
        refresh_secret: &SecretHandle,
    ) -> Result<(), AuthProductError>;

    async fn cleanup_exchange_material(
        &self,
        scope: &ResourceScope,
        flow_id: AuthFlowId,
    ) -> Result<(), AuthProductError>;
}

#[derive(Clone)]
struct StaticOAuthClientMaterialSource {
    material: OAuthClientMaterial,
}

impl fmt::Debug for StaticOAuthClientMaterialSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StaticOAuthClientMaterialSource")
            .field("client_id", &self.material.client_id)
            .field("redirect_uri", &self.material.redirect_uri)
            .field("token_endpoint", &self.material.token_endpoint)
            .field(
                "client_secret",
                &self.material.client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[async_trait]
impl OAuthClientMaterialSource for StaticOAuthClientMaterialSource {
    async fn exchange_material(
        &self,
        _scope: &ResourceScope,
        _flow_id: AuthFlowId,
    ) -> Result<OAuthClientMaterial, AuthProductError> {
        Ok(self.material.clone())
    }

    async fn refresh_material(
        &self,
        _scope: &ResourceScope,
        _refresh_secret: &SecretHandle,
    ) -> Result<OAuthClientMaterial, AuthProductError> {
        Ok(self.material.clone())
    }

    async fn bind_refresh_material(
        &self,
        _scope: &ResourceScope,
        _flow_id: AuthFlowId,
        _refresh_secret: &SecretHandle,
    ) -> Result<(), AuthProductError> {
        Ok(())
    }

    async fn bind_rotated_refresh_material(
        &self,
        _scope: &ResourceScope,
        _previous: &SecretHandle,
        _replacement: &SecretHandle,
    ) -> Result<(), AuthProductError> {
        Ok(())
    }

    async fn cleanup_refresh_material(
        &self,
        _scope: &ResourceScope,
        _refresh_secret: &SecretHandle,
    ) -> Result<(), AuthProductError> {
        Ok(())
    }

    async fn cleanup_exchange_material(
        &self,
        _scope: &ResourceScope,
        _flow_id: AuthFlowId,
    ) -> Result<(), AuthProductError> {
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct HostOAuthProviderClient {
    spec: HostOAuthProviderSpec,
    egress: Arc<dyn RuntimeHttpEgress>,
    #[cfg(feature = "root-llm-provider")]
    #[allow(dead_code)]
    original_egress: Option<Arc<dyn brassclaw_host_runtime::HostHttpOriginalEgress>>,
    secret_store: Arc<dyn SecretStore>,
    obligation_handler: Arc<dyn CapabilityObligationHandler>,
    client_material: Arc<dyn OAuthClientMaterialSource>,
    static_client_material: Option<OAuthClientMaterial>,
    runtime: RuntimeKind,
    capability_id: CapabilityId,
    primitive_settings: Option<Arc<dyn brassclaw_host_runtime::HostPrimitiveSettingsSource>>,
}

impl HostOAuthProviderClient {
    pub(crate) fn new(
        spec: HostOAuthProviderSpec,
        egress: Arc<dyn RuntimeHttpEgress>,
        secret_store: Arc<dyn SecretStore>,
        obligation_handler: Arc<dyn CapabilityObligationHandler>,
        client_id: OAuthClientId,
        redirect_uri: OAuthRedirectUri,
    ) -> Result<Self, AuthProductError> {
        let capability_id = CapabilityId::new(spec.capability_id)
            .map_err(|_| AuthProductError::BackendUnavailable)?;
        oauth_endpoint_host(spec.token_endpoint)?;
        let material = OAuthClientMaterial {
            client_id,
            client_secret: None,
            redirect_uri,
            token_endpoint: spec.token_endpoint.to_string(),
        };
        Ok(Self {
            spec,
            egress,
            #[cfg(feature = "root-llm-provider")]
            original_egress: None,
            secret_store,
            obligation_handler,
            client_material: Arc::new(StaticOAuthClientMaterialSource {
                material: material.clone(),
            }),
            static_client_material: Some(material),
            runtime: RuntimeKind::System,
            capability_id,
            primitive_settings: None,
        })
    }

    pub(crate) fn new_with_client_material(
        spec: HostOAuthProviderSpec,
        egress: Arc<dyn RuntimeHttpEgress>,
        secret_store: Arc<dyn SecretStore>,
        obligation_handler: Arc<dyn CapabilityObligationHandler>,
        client_material: Arc<dyn OAuthClientMaterialSource>,
    ) -> Result<Self, AuthProductError> {
        let capability_id = CapabilityId::new(spec.capability_id)
            .map_err(|_| AuthProductError::BackendUnavailable)?;
        oauth_endpoint_host(spec.token_endpoint)?;
        Ok(Self {
            spec,
            egress,
            #[cfg(feature = "root-llm-provider")]
            original_egress: None,
            secret_store,
            obligation_handler,
            client_material,
            static_client_material: None,
            runtime: RuntimeKind::System,
            capability_id,
            primitive_settings: None,
        })
    }

    /// Native composition supplies the same acknowledged owner used by Monty.
    /// Host-only local-dev composition has no such owner and uses canonical
    /// defaults. A configured but unavailable owner never falls back.
    pub(crate) fn with_primitive_settings_source(
        mut self,
        source: Option<Arc<dyn brassclaw_host_runtime::HostPrimitiveSettingsSource>>,
    ) -> Self {
        self.primitive_settings = source;
        self
    }

    pub(crate) fn with_client_secret(mut self, client_secret: SecretString) -> Self {
        let Some(mut material) = self.static_client_material.clone() else {
            return self;
        };
        material.client_secret = Some(client_secret);
        self.client_material = Arc::new(StaticOAuthClientMaterialSource {
            material: material.clone(),
        });
        self.static_client_material = Some(material);
        self
    }

    /// Trusted composition only; never selected by a Serde field or a runtime
    /// request flag. Root must supply the SAME real host HTTP service instance.
    #[cfg(feature = "root-llm-provider")]
    pub(crate) fn with_original_egress(
        mut self,
        egress: Option<Arc<dyn brassclaw_host_runtime::HostHttpOriginalEgress>>,
    ) -> Self {
        self.original_egress = egress;
        self
    }
    #[cfg(feature = "root-llm-provider")]
    pub(crate) fn native_original_egress(
        &self,
    ) -> Result<Arc<dyn brassclaw_host_runtime::HostHttpOriginalEgress>, AuthProductError> {
        self.original_egress
            .clone()
            .ok_or(AuthProductError::BackendUnavailable)
    }
    /// Native protocol stages reuse the existing host egress/settings path.
    /// A successful raw return must be durably retained by the flow owner before
    /// parsing/publication. This method itself creates no request claim.
    pub(crate) async fn prepare_oauth_http_request(
        &self,
        scope: ResourceScope,
        token_endpoint: &str,
        content_type: &str,
        body: Vec<u8>,
    ) -> Result<RuntimeHttpEgressRequest, AuthProductError> {
        let token_host = oauth_endpoint_host(token_endpoint)?;
        // Capture one coherent, acknowledged revision before any network effect.
        // Authorizing/waiting below cannot mix a newer timeout with older bytes.
        let limits = acknowledged_oauth_limits(self.primitive_settings.as_ref()).await?;
        let timeout_ms = limits.oauth_token_timeout_millis;
        let response_body_limit = limits.oauth_token_response_body_bytes;
        let policy = oauth_network_policy(&token_host, response_body_limit);
        authorize_oauth_egress(
            Arc::clone(&self.obligation_handler),
            &scope,
            &self.capability_id,
            &policy,
        )
        .await?;
        Ok(RuntimeHttpEgressRequest {
            runtime: self.runtime,
            scope,
            capability_id: self.capability_id.clone(),
            method: NetworkMethod::Post,
            url: token_endpoint.to_string(),
            headers: vec![
                ("content-type".to_string(), content_type.to_string()),
                ("accept".to_string(), "application/json".to_string()),
            ],
            body,
            network_policy: policy,
            credential_injections: Vec::<RuntimeCredentialInjection>::new(),
            response_body_limit: Some(response_body_limit),
            save_body_to: None,
            timeout_ms: Some(timeout_ms),
        })
    }
    pub(crate) async fn execute_oauth_http(
        &self,
        scope: ResourceScope,
        token_endpoint: &str,
        content_type: &str,
        body: Vec<u8>,
    ) -> Result<brassclaw_host_api::RuntimeHttpEgressResponse, AuthProductError> {
        let request = self
            .prepare_oauth_http_request(scope, token_endpoint, content_type, body)
            .await?;
        let body_limit = request
            .response_body_limit
            .ok_or(AuthProductError::BackendUnavailable)?;
        let response = self
            .egress
            .execute(request)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?;
        if response.body.len() as u64 > body_limit || response.saved_body.is_some() {
            return Err(AuthProductError::BackendUnavailable);
        }
        Ok(response)
    }
    pub(crate) fn native_device_destinations(
        &self,
        flow_id: AuthFlowId,
        invocation_id: brassclaw_host_api::InvocationId,
    ) -> Result<brassclaw_auth::OAuthTokenDestinations, AuthProductError> {
        let selected = TokenDestinations::exchange(&self.spec, flow_id, invocation_id)?;
        Ok(brassclaw_auth::OAuthTokenDestinations {
            access_secret: selected.access,
            refresh_secret: selected.refresh,
        })
    }
    #[cfg(feature = "root-llm-provider")]
    pub(crate) fn native_device_exchange_body(
        &self,
        client_id: &str,
        redirect_uri: &str,
        code: &str,
        verifier: &str,
    ) -> Result<Vec<u8>, AuthProductError> {
        if self.spec.resource.is_some() {
            return Err(AuthProductError::MalformedConfig);
        }
        Ok(authorization_code_body(
            &self.spec,
            client_id,
            redirect_uri,
            None,
            code,
            verifier,
        ))
    }

    /// Native host-instance refresh uses the same OAuth protocol encoder as
    /// scoped refresh. The caller still owns its authenticated original claim,
    /// final request/response custody and atomic token publication.
    #[cfg(feature = "root-llm-provider")]
    pub(crate) fn native_refresh_body(
        &self,
        client_id: &str,
        refresh_token: &str,
    ) -> Result<Vec<u8>, AuthProductError> {
        if self.spec.resource.is_some() {
            return Err(AuthProductError::MalformedConfig);
        }
        Ok(refresh_body(&self.spec, client_id, None, refresh_token))
    }

    async fn execute_token_request(
        &self,
        scope: ResourceScope,
        token_endpoint: &str,
        body: Vec<u8>,
        refresh_request: bool,
    ) -> Result<OAuthTokenResponse, AuthProductError> {
        let response = self
            .execute_oauth_http(
                scope,
                token_endpoint,
                "application/x-www-form-urlencoded",
                body,
            )
            .await?;
        if !(200..300).contains(&response.status) {
            return Err(if (500..600).contains(&response.status) {
                AuthProductError::BackendUnavailable
            } else if refresh_request {
                AuthProductError::RefreshFailed
            } else {
                AuthProductError::TokenExchangeFailed
            });
        }
        parse_token_response(&response.body).map_err(|error| {
            if refresh_request {
                match error {
                    AuthProductError::BackendUnavailable => AuthProductError::BackendUnavailable,
                    _ => AuthProductError::RefreshFailed,
                }
            } else {
                error
            }
        })
    }

    async fn store_tokens(
        &self,
        scope: ResourceScope,
        destinations: TokenDestinations,
        tokens: OAuthTokenResponse,
        request_started_at: brassclaw_auth::Timestamp,
    ) -> Result<StoredOAuthTokens, AuthProductError> {
        let access_expires_at = access_deadline(request_started_at, tokens.expires_in_seconds)?;
        let refresh_secret = tokens.refresh_token.as_ref().map(|_| destinations.refresh);
        self.store_token_pair(
            scope,
            destinations.access,
            refresh_secret,
            tokens,
            access_expires_at,
        )
        .await
    }

    async fn load_refresh_token(
        &self,
        source: &brassclaw_auth::OAuthRefreshSource,
    ) -> Result<SecretString, AuthProductError> {
        let lease = self
            .secret_store
            .lease_once_if_record_reference(&source.scope, &source.handle, &source.record_reference)
            .await
            .map_err(map_refresh_secret_error)?
            .ok_or(AuthProductError::BackendConflict)?;
        self.secret_store
            .consume(&source.scope, lease.id)
            .await
            .map_err(map_refresh_secret_error)
    }

    async fn store_token_pair(
        &self,
        scope: ResourceScope,
        access_secret: SecretHandle,
        refresh_secret: Option<SecretHandle>,
        tokens: OAuthTokenResponse,
        access_expires_at: Option<brassclaw_auth::Timestamp>,
    ) -> Result<StoredOAuthTokens, AuthProductError> {
        let refresh_token = tokens.refresh_token;
        self.secret_store
            .put_with_expiry(
                scope.clone(),
                access_secret.clone(),
                tokens.access_token,
                access_expires_at,
            )
            .await
            .map_err(map_secret_store_error)?;
        let refresh_secret = match (refresh_secret, refresh_token) {
            (Some(handle), Some(refresh_token)) => {
                if let Err(error) = self
                    .secret_store
                    .put(scope.clone(), handle.clone(), refresh_token)
                    .await
                {
                    cleanup_written_access(
                        &self.secret_store,
                        &scope,
                        &access_secret,
                        self.spec.provider_id,
                    )
                    .await;
                    return Err(map_secret_store_error(error));
                }
                Some(handle)
            }
            (None, None) => None,
            _ => return Err(AuthProductError::BackendUnavailable),
        };
        Ok(StoredOAuthTokens {
            access_secret,
            refresh_secret,
            access_expires_at,
        })
    }

    async fn cleanup_unpublished_tokens(
        &self,
        scope: &ResourceScope,
        tokens: &StoredOAuthTokens,
    ) -> Result<(), AuthProductError> {
        let mut handles = vec![tokens.access_secret.clone()];
        handles.extend(tokens.refresh_secret.clone());
        let token_cleanup = self.delete_tokens(scope, &handles).await;
        let material_cleanup = match &tokens.refresh_secret {
            Some(handle) => {
                self.client_material
                    .cleanup_refresh_material(scope, handle)
                    .await
            }
            None => Ok(()),
        };
        if let Err(error) = &token_cleanup {
            tracing::debug!(code = ?error.code(), "Unpublished OAuth token cleanup failed");
        }
        if let Err(error) = &material_cleanup {
            tracing::debug!(code = ?error.code(), "Unpublished OAuth client-material cleanup failed");
        }
        token_cleanup?;
        material_cleanup
    }

    async fn delete_tokens(
        &self,
        scope: &ResourceScope,
        handles: &[SecretHandle],
    ) -> Result<(), AuthProductError> {
        let mut first_error = None;
        for handle in handles {
            if let Err(error) = self.secret_store.delete(scope, handle).await
                && first_error.is_none()
            {
                first_error = Some(map_secret_store_error(error));
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

impl fmt::Debug for HostOAuthProviderClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostOAuthProviderClient")
            .field("provider_id", &self.spec.provider_id)
            .field("client_material", &self.client_material)
            .field("runtime", &self.runtime)
            .field("capability_id", &self.capability_id)
            .field("acknowledged_settings", &self.primitive_settings.is_some())
            .finish()
    }
}

#[async_trait]
impl AuthProviderClient for HostOAuthProviderClient {
    async fn prepare_callback(
        &self,
        context: &brassclaw_auth::OAuthProviderExchangeContext,
        request: &brassclaw_auth::OAuthProviderCallbackRequest,
    ) -> Result<brassclaw_auth::OAuthTokenDestinations, AuthProductError> {
        validate_provider_callback_request(request)?;
        if request.provider.as_str() != self.spec.provider_id || context.scope.resource.is_system()
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        self.native_device_destinations(context.flow_id, context.scope.resource.invocation_id)
    }

    async fn prepare_refresh(
        &self,
        request: &OAuthProviderRefreshRequest,
    ) -> Result<brassclaw_auth::OAuthProviderRefreshPreparation, AuthProductError> {
        if request.provider.as_str() != self.spec.provider_id || request.scope.resource.is_system()
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let destinations = TokenDestinations::refresh(
            &self.spec,
            request.account_id,
            request.refresh_id.as_uuid(),
        )?;
        let record_reference = self
            .secret_store
            .record_reference(&request.scope.resource, &request.refresh_secret)
            .await
            .map_err(map_refresh_secret_error)?
            .ok_or(AuthProductError::CredentialMissing)?;
        let source = brassclaw_auth::OAuthRefreshSource {
            scope: request.scope.resource.clone(),
            handle: request.refresh_secret.clone(),
            record_reference,
        };
        source.verify(request)?;
        Ok(brassclaw_auth::OAuthProviderRefreshPreparation {
            destinations: brassclaw_auth::OAuthTokenDestinations {
                access_secret: destinations.access,
                refresh_secret: destinations.refresh,
            },
            source,
        })
    }

    async fn exchange_callback(
        &self,
        context: OAuthProviderExchangeContext,
        request: OAuthProviderCallbackRequest,
    ) -> Result<OAuthProviderExchange, AuthProductError> {
        if request.provider.as_str() != self.spec.provider_id {
            return Err(AuthProductError::TokenExchangeFailed);
        }
        validate_provider_callback_request(&request)?;
        let callback_scope = context.scope.resource.clone();
        if callback_scope.is_system() {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let destinations =
            TokenDestinations::exchange(&self.spec, context.flow_id, callback_scope.invocation_id)?;
        let material = self
            .client_material
            .exchange_material(&callback_scope, context.flow_id)
            .await?;
        let body = authorization_code_body(
            &self.spec,
            material.client_id.as_str(),
            material.redirect_uri.as_str(),
            material.client_secret.as_ref(),
            request.authorization_code.expose_secret(),
            request.pkce_verifier.expose_secret(),
        );
        let request_started_at = chrono::Utc::now();
        let token_response = self
            .execute_token_request(
                callback_scope.clone(),
                &material.token_endpoint,
                body,
                false,
            )
            .await?;
        let scopes = scopes_for_exchange(&self.spec, &token_response, &request.scopes)?;
        let stored_tokens = self
            .store_tokens(
                callback_scope,
                destinations,
                token_response,
                request_started_at,
            )
            .await?;
        if let Some(refresh_secret) = &stored_tokens.refresh_secret
            && let Err(error) = self
                .client_material
                .bind_refresh_material(&context.scope.resource, context.flow_id, refresh_secret)
                .await
        {
            tracing::debug!(code = ?error.code(), "Initial OAuth client binding failed before account publication");
            let token_cleanup = self
                .cleanup_unpublished_tokens(&context.scope.resource, &stored_tokens)
                .await;
            let flow_cleanup = self
                .client_material
                .cleanup_exchange_material(&context.scope.resource, context.flow_id)
                .await;
            if let Err(cleanup_error) = &flow_cleanup {
                tracing::debug!(code = ?cleanup_error.code(), "Unpublished OAuth flow-material cleanup failed");
            }
            token_cleanup?;
            flow_cleanup?;
            return Err(error);
        }

        Ok(OAuthProviderExchange {
            provider: request.provider,
            account_label: request.account_label,
            authorization_code_hash: request.authorization_code_hash,
            pkce_verifier_hash: request.pkce_verifier_hash,
            access_secret: stored_tokens.access_secret,
            access_expires_at: stored_tokens.access_expires_at,
            refresh_secret: stored_tokens.refresh_secret,
            scopes,
            account_id: None,
        })
    }

    async fn refresh_token(
        &self,
        request: OAuthProviderRefreshRequest,
        preparation: brassclaw_auth::OAuthProviderRefreshPreparation,
    ) -> Result<OAuthProviderRefresh, AuthProductError> {
        if request.provider.as_str() != self.spec.provider_id {
            return Err(AuthProductError::RefreshFailed);
        }
        preparation.source.verify(&request)?;
        let refresh_scope = request.scope.resource.clone();
        if refresh_scope.is_system() {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let targets = TokenDestinations::refresh(
            &self.spec,
            request.account_id,
            request.refresh_id.as_uuid(),
        )?;
        if targets.access != preparation.destinations.access_secret
            || targets.refresh != preparation.destinations.refresh_secret
        {
            return Err(AuthProductError::BackendConflict);
        }
        let refresh_token = self.load_refresh_token(&preparation.source).await?;
        let material = self
            .client_material
            .refresh_material(&refresh_scope, &request.refresh_secret)
            .await?;
        let body = refresh_body(
            &self.spec,
            material.client_id.as_str(),
            material.client_secret.as_ref(),
            refresh_token.expose_secret(),
        );
        let request_started_at = chrono::Utc::now();
        let token_response = self
            .execute_token_request(refresh_scope.clone(), &material.token_endpoint, body, true)
            .await?;
        let scopes = scopes_for_refresh(&token_response, &request.scopes);
        let stored_tokens = self
            .store_tokens(
                refresh_scope.clone(),
                targets,
                token_response,
                request_started_at,
            )
            .await
            .map_err(|error| match error {
                AuthProductError::TokenExchangeFailed => AuthProductError::RefreshFailed,
                error => error,
            })?;
        if let Some(replacement) = &stored_tokens.refresh_secret
            && let Err(error) = self
                .client_material
                .bind_rotated_refresh_material(&refresh_scope, &request.refresh_secret, replacement)
                .await
        {
            // These fresh handles have not been returned or published to
            // an account. Cleanup cannot delete another refresh's tokens.
            tracing::debug!(code = ?error.code(), "Rotated OAuth client binding failed before account publication");
            self.cleanup_unpublished_tokens(&refresh_scope, &stored_tokens)
                .await?;
            return Err(error);
        }

        Ok(OAuthProviderRefresh {
            provider: request.provider,
            access_secret: stored_tokens.access_secret,
            access_expires_at: stored_tokens.access_expires_at,
            refresh_secret: stored_tokens.refresh_secret,
            scopes,
        })
    }

    async fn cleanup_exchange(
        &self,
        context: OAuthProviderExchangeContext,
        exchange: &OAuthProviderExchange,
    ) -> Result<(), AuthProductError> {
        if exchange.provider.as_str() != self.spec.provider_id {
            return Ok(());
        }
        let mut handles = vec![exchange.access_secret.clone()];
        handles.extend(exchange.refresh_secret.clone());
        self.delete_tokens(&context.scope.resource, &handles)
            .await?;
        self.client_material
            .cleanup_exchange_material(&context.scope.resource, context.flow_id)
            .await
    }
}

struct TokenDestinations {
    access: SecretHandle,
    refresh: SecretHandle,
}
impl TokenDestinations {
    fn refresh(
        spec: &HostOAuthProviderSpec,
        account_id: CredentialAccountId,
        write_id: uuid::Uuid,
    ) -> Result<Self, AuthProductError> {
        Ok(Self {
            access: refresh_token_handle(spec, account_id, write_id, "access")?,
            refresh: refresh_token_handle(spec, account_id, write_id, "refresh")?,
        })
    }
    fn exchange(
        spec: &HostOAuthProviderSpec,
        flow_id: AuthFlowId,
        invocation_id: brassclaw_host_api::InvocationId,
    ) -> Result<Self, AuthProductError> {
        Ok(Self {
            access: exchange_token_handle(spec, flow_id, invocation_id, "access")?,
            refresh: exchange_token_handle(spec, flow_id, invocation_id, "refresh")?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredOAuthTokens {
    access_secret: SecretHandle,
    refresh_secret: Option<SecretHandle>,
    access_expires_at: Option<brassclaw_auth::Timestamp>,
}

fn access_deadline(
    request_started_at: brassclaw_auth::Timestamp,
    expires_in_seconds: Option<u64>,
) -> Result<Option<brassclaw_auth::Timestamp>, AuthProductError> {
    let Some(seconds) = expires_in_seconds else {
        return Ok(None);
    };
    let duration = i64::try_from(seconds)
        .ok()
        .and_then(chrono::Duration::try_seconds)
        .ok_or(AuthProductError::TokenExchangeFailed)?;
    request_started_at
        .checked_add_signed(duration)
        .map(Some)
        .ok_or(AuthProductError::TokenExchangeFailed)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct TokenResponseBody {
    access_token: SecretString,
    #[serde(default)]
    refresh_token: Option<SecretString>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default, deserialize_with = "deserialize_lifetime_seconds")]
    expires_in: Option<u64>,
    #[serde(default)]
    token_type: Option<String>,
}

fn deserialize_lifetime_seconds<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u64>, D::Error> {
    // Missing is unknown. An explicitly supplied value must be a JSON integer;
    // null must not silently turn a finite/invalid response into unknown expiry.
    u64::deserialize(deserializer).map(Some)
}

fn parse_token_response(body: &[u8]) -> Result<OAuthTokenResponse, AuthProductError> {
    let parsed: TokenResponseBody =
        serde_json::from_slice(body).map_err(|_| AuthProductError::TokenExchangeFailed)?;
    let response_scope = parsed
        .scope
        .as_deref()
        .filter(|scope| !scope.trim().is_empty());
    let _ = parsed.token_type;
    OAuthTokenResponse::new(
        parsed.access_token,
        parsed.refresh_token,
        response_scope,
        parsed.expires_in,
    )
    .map_err(|_| AuthProductError::TokenExchangeFailed)
}

fn scopes_for_exchange(
    spec: &HostOAuthProviderSpec,
    token_response: &OAuthTokenResponse,
    requested_scopes: &[ProviderScope],
) -> Result<Vec<ProviderScope>, AuthProductError> {
    if !token_response.scopes.is_empty() {
        return Ok(token_response.scopes.clone());
    }
    match spec.exchange_scope_policy {
        ExchangeScopePolicy::RequireProviderScope => Err(AuthProductError::TokenExchangeFailed),
        ExchangeScopePolicy::FallbackToRequested => Ok(requested_scopes.to_vec()),
    }
}

fn scopes_for_refresh(
    token_response: &OAuthTokenResponse,
    existing_scopes: &[ProviderScope],
) -> Vec<ProviderScope> {
    if token_response.scopes.is_empty() {
        existing_scopes.to_vec()
    } else {
        token_response.scopes.clone()
    }
}

fn authorization_code_body(
    spec: &HostOAuthProviderSpec,
    client_id: &str,
    redirect_uri: &str,
    client_secret: Option<&SecretString>,
    code: &str,
    pkce_verifier: &str,
) -> Vec<u8> {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer
        .append_pair("grant_type", "authorization_code")
        .append_pair("code", code)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("client_id", client_id)
        .append_pair("code_verifier", pkce_verifier);
    append_resource(spec, &mut serializer);
    if let Some(client_secret) = client_secret {
        serializer.append_pair("client_secret", client_secret.expose_secret());
    }
    serializer.finish().into_bytes()
}

fn refresh_body(
    spec: &HostOAuthProviderSpec,
    client_id: &str,
    client_secret: Option<&SecretString>,
    refresh_token: &str,
) -> Vec<u8> {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer
        .append_pair("grant_type", "refresh_token")
        .append_pair("refresh_token", refresh_token)
        .append_pair("client_id", client_id);
    append_resource(spec, &mut serializer);
    if let Some(client_secret) = client_secret {
        serializer.append_pair("client_secret", client_secret.expose_secret());
    }
    serializer.finish().into_bytes()
}

fn append_resource(
    spec: &HostOAuthProviderSpec,
    serializer: &mut url::form_urlencoded::Serializer<String>,
) {
    if let Some(resource) = spec.resource {
        serializer.append_pair("resource", resource);
    }
}

fn exchange_token_handle(
    spec: &HostOAuthProviderSpec,
    flow_id: AuthFlowId,
    invocation_id: brassclaw_host_api::InvocationId,
    token_kind: &'static str,
) -> Result<SecretHandle, AuthProductError> {
    SecretHandle::new(format!(
        "{}-oauth-{token_kind}-{flow_id}-{invocation_id}",
        spec.secret_handle_prefix
    ))
    .map_err(|_| AuthProductError::BackendUnavailable)
}

fn refresh_token_handle(
    spec: &HostOAuthProviderSpec,
    account_id: CredentialAccountId,
    write_id: uuid::Uuid,
    token_kind: &'static str,
) -> Result<SecretHandle, AuthProductError> {
    SecretHandle::new(format!(
        "{}-oauth-refresh-{token_kind}-{account_id}-{write_id}",
        spec.secret_handle_prefix
    ))
    .map_err(|_| AuthProductError::BackendUnavailable)
}

pub(crate) fn oauth_endpoint_host(endpoint: &str) -> Result<String, AuthProductError> {
    let url = url::Url::parse(endpoint).map_err(|_| AuthProductError::BackendUnavailable)?;
    if url.scheme() != "https" {
        return Err(AuthProductError::BackendUnavailable);
    }
    url.host_str()
        .filter(|host| !host.trim().is_empty())
        .map(str::to_string)
        .ok_or(AuthProductError::BackendUnavailable)
}

pub(crate) fn oauth_network_policy(token_host: &str, response_body_limit: u64) -> NetworkPolicy {
    NetworkPolicy {
        allowed_targets: vec![NetworkTargetPattern {
            scheme: Some(NetworkScheme::Https),
            host_pattern: token_host.to_string(),
            port: None,
        }],
        deny_private_ip_ranges: true,
        max_egress_bytes: Some(response_body_limit),
    }
}

pub(crate) async fn authorize_oauth_egress(
    handler: Arc<dyn CapabilityObligationHandler>,
    scope: &ResourceScope,
    capability_id: &CapabilityId,
    policy: &NetworkPolicy,
) -> Result<(), AuthProductError> {
    let context = oauth_execution_context(scope.clone())?;
    let estimate = ResourceEstimate {
        network_egress_bytes: policy.max_egress_bytes,
        ..ResourceEstimate::default()
    };
    handler
        .satisfy(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id,
            estimate: &estimate,
            obligations: &[Obligation::ApplyNetworkPolicy {
                policy: policy.clone(),
            }],
        })
        .await
        .map_err(|_| AuthProductError::BackendUnavailable)
}

fn oauth_execution_context(
    resource_scope: ResourceScope,
) -> Result<brassclaw_host_api::ExecutionContext, AuthProductError> {
    let context = brassclaw_host_api::ExecutionContext {
        invocation_id: resource_scope.invocation_id,
        correlation_id: CorrelationId::new(),
        process_id: None,
        parent_process_id: None,
        tenant_id: resource_scope.tenant_id.clone(),
        user_id: resource_scope.user_id.clone(),
        agent_id: resource_scope.agent_id.clone(),
        project_id: resource_scope.project_id.clone(),
        thread_id: resource_scope.thread_id.clone(),
        extension_id: ExtensionId::new("brassclaw_auth")
            .map_err(|_| AuthProductError::BackendUnavailable)?,
        runtime: RuntimeKind::System,
        trust: TrustClass::System,
        grants: CapabilitySet::default(),
        mounts: MountView::default(),
        resource_scope,
    };
    context
        .validate()
        .map_err(|_| AuthProductError::BackendUnavailable)?;
    Ok(context)
}

async fn cleanup_written_access(
    store: &Arc<dyn SecretStore>,
    scope: &ResourceScope,
    access_secret: &SecretHandle,
    provider_id: &str,
) {
    if let Err(delete_error) = store.delete(scope, access_secret).await {
        tracing::warn!(
            provider_id,
            secret_store_reason = delete_error.stable_reason(),
            "oauth callback cleanup failed after refresh token write failure"
        );
    }
}

fn map_secret_store_error(error: brassclaw_secrets::SecretStoreError) -> AuthProductError {
    tracing::debug!(
        secret_store_reason = error.stable_reason(),
        "oauth provider secret store operation failed"
    );
    AuthProductError::BackendUnavailable
}

fn map_refresh_secret_error(error: brassclaw_secrets::SecretStoreError) -> AuthProductError {
    if error.is_unknown_secret()
        || error.is_unknown_lease()
        || error.is_consumed()
        || error.is_revoked()
        || error.is_expired()
    {
        AuthProductError::RefreshFailed
    } else {
        tracing::debug!(
            secret_store_reason = error.stable_reason(),
            "oauth provider refresh secret load failed"
        );
        AuthProductError::BackendUnavailable
    }
}

#[cfg(test)]
mod tests;
