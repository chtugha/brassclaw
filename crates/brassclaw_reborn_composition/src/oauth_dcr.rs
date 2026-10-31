use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use brassclaw_auth::{
    AuthChallenge, AuthContinuationRef, AuthFlowId, AuthFlowManager, AuthFlowOwnerScope,
    AuthFlowRecordSource, AuthGateRef, AuthProductError, AuthProductScope, AuthProviderId,
    CredentialAccountLabel, CredentialAccountUpdateBinding, OAuthAuthorizationEndpoint,
    OAuthAuthorizeUrlRequest, OAuthClientId, OAuthExtraParam, OAuthRedirectUri, OAuthState,
    PkceVerifierSecret, ProviderScope, TurnGateAuthFlowQuery, TurnRunRef, build_authorization_url,
    opaque_state_hash, pkce_s256_challenge, pkce_verifier_hash,
};
use brassclaw_capabilities::CapabilityObligationHandler;
use brassclaw_host_api::{
    CapabilityId, InvocationId, NetworkMethod, ResourceScope, RuntimeCredentialAuthRequirement,
    RuntimeHttpEgress, RuntimeHttpEgressRequest, RuntimeKind, SecretHandle,
};
use brassclaw_product_adapters::AuthPromptChallengeKind;
use brassclaw_secrets::{SecretMaterial, SecretStore};
use brassclaw_turns::{TurnRunId, TurnScope};
use chrono::{Duration as ChronoDuration, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[cfg(test)]
#[path = "oauth_dcr_cleanup_tests.rs"]
mod cleanup_tests;

#[path = "oauth_dcr_private.rs"]
mod private_material;

#[cfg(test)]
#[path = "oauth_dcr_legacy_tests.rs"]
mod legacy_tests;

#[path = "oauth_dcr_legacy.rs"]
mod legacy;

#[path = "oauth_dcr_retirement.rs"]
mod retirement;

#[path = "oauth_dcr_cleanup.rs"]
mod cleanup;

#[path = "oauth_dcr_original.rs"]
mod original;
use original::DcrOriginalStart;

use crate::auth_prompt::AuthChallengeView;
use crate::oauth_dcr_protocol::{
    AuthorizationServerMetadata, DcrRegistrationRequest, DcrRegistrationResponse,
    ProtectedResourceMetadata, StoredDcrClientMaterial, authorization_server_metadata_url,
    authorization_server_metadata_url_from_issuer, callback_base_url, flow_secret_handle,
    protected_resource_metadata_url, refresh_secret_handle, scope_text, validate_callback_origin,
    validate_endpoint_origin, validate_issuer_related_to_resource,
};
use crate::oauth_provider_client::{
    HostOAuthProviderSpec, OAuthClientMaterial, OAuthClientMaterialSource,
    acknowledged_oauth_limits, authorize_oauth_egress, oauth_endpoint_host, oauth_network_policy,
};

const DCR_FLOW_TTL_SECONDS: i64 = 600;

/// One accepted HTTP request retains a coherent policy across asynchronous work.
/// This is technical request data, not an execution/registration permission.
#[derive(Debug, Clone, Copy)]
struct CapturedDcrHttpLimits {
    timeout_millis: u32,
    response_body_bytes: u64,
}

#[cfg(all(test, feature = "skills-db"))]
#[path = "oauth_dcr_limits_tests.rs"]
mod limits_tests;

#[derive(Debug, Clone)]
pub(crate) struct OAuthDcrProviderConfig {
    pub(crate) spec: HostOAuthProviderSpec,
    pub(crate) callback_origin: String,
    pub(crate) client_name: String,
    pub(crate) account_label: CredentialAccountLabel,
    pub(crate) scopes: Vec<ProviderScope>,
}

#[derive(Clone)]
pub(crate) struct OAuthDcrProvider {
    spec: HostOAuthProviderSpec,
    callback_origin: String,
    client_name: String,
    account_label: CredentialAccountLabel,
    scopes: Vec<ProviderScope>,
    egress: Arc<dyn RuntimeHttpEgress>,
    secret_store: Arc<dyn SecretStore>,
    obligation_handler: Arc<dyn CapabilityObligationHandler>,
    capability_id: CapabilityId,
    metadata_cache: Arc<RwLock<Option<CachedAuthorizationServerMetadata>>>,
    primitive_settings: Option<Arc<dyn brassclaw_host_runtime::HostPrimitiveSettingsSource>>,
}

impl OAuthDcrProvider {
    pub(crate) fn new(
        config: OAuthDcrProviderConfig,
        egress: Arc<dyn RuntimeHttpEgress>,
        secret_store: Arc<dyn SecretStore>,
        obligation_handler: Arc<dyn CapabilityObligationHandler>,
    ) -> Result<Self, AuthProductError> {
        validate_callback_origin(&config.callback_origin)?;
        let capability_id = CapabilityId::new(config.spec.capability_id)
            .map_err(|_| AuthProductError::BackendUnavailable)?;
        Ok(Self {
            spec: config.spec,
            callback_origin: config.callback_origin,
            client_name: config.client_name,
            account_label: config.account_label,
            scopes: config.scopes,
            egress,
            secret_store,
            obligation_handler,
            capability_id,
            metadata_cache: Arc::new(RwLock::new(None)),
            primitive_settings: None,
        })
    }

    pub(crate) fn with_primitive_settings_source(
        mut self,
        source: Option<Arc<dyn brassclaw_host_runtime::HostPrimitiveSettingsSource>>,
    ) -> Self {
        self.primitive_settings = source;
        self
    }

    async fn capture_http_limits(&self) -> Result<CapturedDcrHttpLimits, AuthProductError> {
        let limits = acknowledged_oauth_limits(self.primitive_settings.as_ref()).await?;
        Ok(CapturedDcrHttpLimits {
            timeout_millis: limits.oauth_dcr_timeout_millis,
            response_body_bytes: limits.oauth_dcr_response_body_bytes,
        })
    }

    pub(crate) fn spec(&self) -> &HostOAuthProviderSpec {
        &self.spec
    }

    pub(crate) async fn challenge_for_blocked_gate(
        &self,
        flow_manager: &Arc<dyn AuthFlowManager>,
        flow_source: &Arc<dyn AuthFlowRecordSource>,
        scope: &TurnScope,
        owner_user_id: &brassclaw_host_api::UserId,
        run_id: TurnRunId,
        gate_ref: &AuthGateRef,
    ) -> Result<AuthChallengeView, AuthProductError> {
        let mut auth_scope = auth_scope_for_blocked_turn(scope, owner_user_id);
        let seed = serde_json::to_vec(&(
            "brassclaw-dcr-gate-original/1",
            scope.tenant_id.as_str(),
            owner_user_id.as_str(),
            &scope.agent_id,
            &scope.project_id,
            scope.thread_id.as_str(),
            run_id.to_string(),
            gate_ref.as_str(),
            self.spec.provider_id,
        ))
        .map_err(|_| AuthProductError::BackendUnavailable)?;
        let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, &seed);
        auth_scope.resource.invocation_id =
            InvocationId::from_uuid(uuid::Uuid::new_v5(&id, b"dcr-auth-invocation/1"));
        let flow_id = AuthFlowId::from_uuid(id);
        let turn_run_ref = TurnRunRef::new(run_id.to_string())?;
        let query = TurnGateAuthFlowQuery {
            owner: AuthFlowOwnerScope {
                tenant_id: auth_scope.resource.tenant_id.clone(),
                user_id: auth_scope.resource.user_id.clone(),
                agent_id: auth_scope.resource.agent_id.clone(),
                project_id: auth_scope.resource.project_id.clone(),
                thread_id: scope.thread_id.clone(),
            },
            turn_run_ref: turn_run_ref.clone(),
            gate_ref: gate_ref.clone(),
            include_terminal: true,
        };
        if let Some(existing) = flow_source.flow_for_turn_gate(query).await? {
            if existing.provider.as_str() != self.spec.provider_id {
                return Err(AuthProductError::CrossScopeDenied);
            }
            let flow = if let Some(retained) = flow_manager
                .read_dcr_registration(&existing.scope, existing.id)
                .await?
            {
                self.continue_original_registration(flow_manager, retained)
                    .await?
            } else {
                self.retain_legacy_flow(flow_manager, existing).await?
            };
            return challenge_view_from_flow(&flow);
        }
        let continuation = AuthContinuationRef::TurnGateResume {
            turn_run_ref,
            gate_ref: gate_ref.clone(),
        };
        // Encrypted preparation can commit before the Auth flow is published.
        // Recover that accepted original before deriving any new expiry, PKCE
        // or provider defaults. Its exact gate provenance still has to agree.
        let request = match self
            .retained_preparation_original(&auth_scope, flow_id)
            .await?
        {
            Some(original) => {
                if original.continuation != continuation
                    || original.requirement_setup.is_some()
                    || original.update_binding.is_some()
                {
                    return Err(AuthProductError::BackendConflict);
                }
                DcrOriginalStart {
                    requirement_setup: None,
                    scope: auth_scope,
                    flow_id,
                    account_label: original.account_label,
                    provider_scopes: original.requested_scopes,
                    continuation,
                    update_binding: None,
                    expires_at: original.expires_at,
                }
            }
            None => DcrOriginalStart {
                requirement_setup: None,
                scope: auth_scope,
                flow_id,
                account_label: self.account_label.clone(),
                provider_scopes: self.scopes.clone(),
                continuation,
                update_binding: None,
                expires_at: Utc::now() + ChronoDuration::seconds(DCR_FLOW_TTL_SECONDS),
            },
        };
        let flow = self
            .start_original_registration(flow_manager, request)
            .await?;
        challenge_view_from_flow(&flow)
    }

    #[allow(
        dead_code,
        reason = "used by the webui-v2-beta OAuth callback route through RebornProductAuthServices"
    )]
    pub(crate) async fn pkce_verifier_for_flow(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<SecretString>, AuthProductError> {
        if let Some(payload) = self.load_legacy_payload(&scope.resource, flow_id).await? {
            return Ok(Some(payload.pkce));
        }
        let handle = flow_secret_handle(&self.spec, flow_id, "pkce")?;
        match self.load_secret(&scope.resource, &handle).await {
            Ok(value) => Ok(Some(value)),
            Err(AuthProductError::UnknownOrExpiredFlow) => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn discover_authorization_server(
        &self,
        scope: &ResourceScope,
    ) -> Result<AuthorizationServerMetadata, AuthProductError> {
        let cache_ttl_secs = acknowledged_oauth_limits(self.primitive_settings.as_ref())
            .await?
            .oauth_dcr_metadata_cache_ttl_secs;
        {
            let mut cache = self.metadata_cache.write().await;
            if let Some(cached) = cache.as_ref()
                && cached.is_current(cache_ttl_secs)
            {
                return Ok(cached.metadata.clone());
            }
            // An observed expired/disabled entry is removed permanently. A later
            // increase cannot resurrect it or extend its original maximum age.
            *cache = None;
        }
        let Some(resource) = self.spec.resource else {
            return Err(AuthProductError::BackendUnavailable);
        };
        let resource_metadata_url = protected_resource_metadata_url(resource)?;
        // Admit the first GET with one coherent acknowledged pair.
        let http_limits = self.capture_http_limits().await?;
        let resource_metadata = self.get_json::<ProtectedResourceMetadata>(
            scope.clone(),
            &resource_metadata_url,
            http_limits,
        );
        let authorization_server_metadata = match resource_metadata.await {
            Ok(metadata) => {
                let issuer = metadata
                    .authorization_servers
                    .into_iter()
                    .next()
                    .ok_or(AuthProductError::BackendUnavailable)?;
                validate_issuer_related_to_resource(resource, &issuer)?;
                authorization_server_metadata_url_from_issuer(&issuer)?
            }
            Err(error) => {
                tracing::debug!(
                    provider = self.spec.provider_id,
                    reason = ?error.code(),
                    "DCR protected-resource metadata unavailable; falling back to issuer well-known URL"
                );
                authorization_server_metadata_url(resource)?
            }
        };
        // This is a distinct HTTP request, whether resource discovery succeeded
        // or selected the legitimate fallback. Use the then-current acknowledged
        // pair, while the earlier accepted GET retains its own captured bounds.
        let http_limits = self.capture_http_limits().await?;
        let metadata = self
            .get_json::<AuthorizationServerMetadata>(
                scope.clone(),
                &authorization_server_metadata,
                http_limits,
            )
            .await?;
        if metadata.registration_endpoint.trim().is_empty() {
            return Err(AuthProductError::BackendUnavailable);
        }
        validate_endpoint_origin(
            &metadata.authorization_endpoint,
            &authorization_server_metadata,
        )?;
        validate_endpoint_origin(&metadata.token_endpoint, &authorization_server_metadata)?;
        validate_endpoint_origin(
            &metadata.registration_endpoint,
            &authorization_server_metadata,
        )?;
        if cache_ttl_secs != 0 {
            *self.metadata_cache.write().await = Some(CachedAuthorizationServerMetadata {
                metadata: metadata.clone(),
                stored_at: Instant::now(),
                ttl_secs: cache_ttl_secs,
            });
        }
        Ok(metadata)
    }

    async fn load_flow_client_material(
        &self,
        scope: &ResourceScope,
        flow_id: AuthFlowId,
    ) -> Result<OAuthClientMaterial, AuthProductError> {
        let material = if let Some(payload) = self.load_legacy_payload(scope, flow_id).await? {
            payload.client
        } else {
            self.load_material(scope, &flow_secret_handle(&self.spec, flow_id, "client")?)
                .await?
        };
        material.into_client_material()
    }

    async fn bind_refresh_material(
        &self,
        scope: &ResourceScope,
        flow_id: AuthFlowId,
        refresh_secret: &SecretHandle,
    ) -> Result<(), AuthProductError> {
        let material = if let Some(payload) = self.load_legacy_payload(scope, flow_id).await? {
            payload.client
        } else {
            self.load_material(scope, &flow_secret_handle(&self.spec, flow_id, "client")?)
                .await?
        };
        self.put_material(
            scope,
            refresh_secret_handle(&self.spec, refresh_secret)?,
            &material,
        )
        .await
    }

    async fn load_refresh_client_material(
        &self,
        scope: &ResourceScope,
        refresh_secret: &SecretHandle,
    ) -> Result<OAuthClientMaterial, AuthProductError> {
        let material = self
            .load_material(scope, &refresh_secret_handle(&self.spec, refresh_secret)?)
            .await?;
        material.into_client_material()
    }

    async fn cleanup_flow_material(
        &self,
        scope: &ResourceScope,
        flow_id: AuthFlowId,
    ) -> Result<(), AuthProductError> {
        if self.cleanup_legacy_inputs(scope, flow_id).await? {
            return Ok(());
        }
        let handles = [
            flow_secret_handle(&self.spec, flow_id, "pkce")?,
            flow_secret_handle(&self.spec, flow_id, "client")?,
        ];
        let mut first_error = None;
        for handle in handles {
            if let Err(error) = self.secret_store.delete(scope, &handle).await
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        first_error.map_or(Ok(()), |_| Err(AuthProductError::BackendUnavailable))
    }

    fn callback_redirect_uri(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
        account_label: &CredentialAccountLabel,
    ) -> Result<OAuthRedirectUri, AuthProductError> {
        let mut url = callback_base_url(&self.callback_origin, flow_id)?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("user_id", scope.resource.user_id.as_str());
            query.append_pair("invocation_id", &scope.resource.invocation_id.to_string());
            query.append_pair("provider", self.spec.provider_id);
            query.append_pair("account_label", account_label.as_str());
            query.append_pair("scope", &scope_text(&self.scopes));
            if let Some(agent_id) = &scope.resource.agent_id {
                query.append_pair("agent_id", agent_id.as_str());
            }
            if let Some(project_id) = &scope.resource.project_id {
                query.append_pair("project_id", project_id.as_str());
            }
            if let Some(thread_id) = &scope.resource.thread_id {
                query.append_pair("thread_id", thread_id.as_str());
            }
            if let Some(session_id) = &scope.session_id {
                query.append_pair("session_id", session_id.as_str());
            }
        }
        OAuthRedirectUri::new(url.to_string())
    }

    async fn get_json<T>(
        &self,
        scope: ResourceScope,
        url: &str,
        http_limits: CapturedDcrHttpLimits,
    ) -> Result<T, AuthProductError>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.execute_json_request(
            scope,
            NetworkMethod::Get,
            url,
            Vec::new(),
            http_limits,
            None,
        )
        .await
    }

    async fn execute_json_request<T>(
        &self,
        scope: ResourceScope,
        method: NetworkMethod,
        url: &str,
        body: Vec<u8>,
        http_limits: CapturedDcrHttpLimits,
        bearer_token: Option<&SecretString>,
    ) -> Result<T, AuthProductError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let response = self
            .execute_request(scope, method, url, body, http_limits, bearer_token)
            .await?;
        if !(200..300).contains(&response.status) {
            return Err(AuthProductError::BackendUnavailable);
        }
        serde_json::from_slice(&response.body).map_err(|_| AuthProductError::BackendUnavailable)
    }

    async fn execute_request(
        &self,
        scope: ResourceScope,
        method: NetworkMethod,
        url: &str,
        body: Vec<u8>,
        http_limits: CapturedDcrHttpLimits,
        bearer_token: Option<&SecretString>,
    ) -> Result<brassclaw_host_api::RuntimeHttpEgressResponse, AuthProductError> {
        let host = oauth_endpoint_host(url)?;
        let policy = oauth_network_policy(&host, http_limits.response_body_bytes);
        authorize_oauth_egress(
            Arc::clone(&self.obligation_handler),
            &scope,
            &self.capability_id,
            &policy,
        )
        .await?;
        let mut headers = vec![
            ("accept".to_string(), "application/json".to_string()),
            ("content-type".to_string(), "application/json".to_string()),
        ];
        if let Some(token) = bearer_token {
            headers.push((
                "authorization".to_string(),
                format!("Bearer {}", token.expose_secret()),
            ));
        }
        let response = self
            .egress
            .execute(RuntimeHttpEgressRequest {
                runtime: RuntimeKind::System,
                scope,
                capability_id: self.capability_id.clone(),
                method,
                url: url.to_string(),
                headers,
                body,
                network_policy: policy,
                credential_injections: Vec::new(),
                response_body_limit: Some(http_limits.response_body_bytes),
                save_body_to: None,
                timeout_ms: Some(http_limits.timeout_millis),
            })
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?;
        Ok(response)
    }

    async fn put_material(
        &self,
        scope: &ResourceScope,
        handle: SecretHandle,
        material: &StoredDcrClientMaterial,
    ) -> Result<(), AuthProductError> {
        let encoded =
            serde_json::to_string(material).map_err(|_| AuthProductError::BackendUnavailable)?;
        self.put_secret(scope, handle, SecretString::from(encoded))
            .await
    }

    async fn load_material(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<StoredDcrClientMaterial, AuthProductError> {
        let material = self.load_secret(scope, handle).await?;
        serde_json::from_str(material.expose_secret())
            .map_err(|_| AuthProductError::BackendUnavailable)
    }

    async fn put_secret(
        &self,
        scope: &ResourceScope,
        handle: SecretHandle,
        material: SecretMaterial,
    ) -> Result<(), AuthProductError> {
        self.secret_store
            .put(scope.clone(), handle, material)
            .await
            .map(|_| ())
            .map_err(|_| AuthProductError::BackendUnavailable)
    }

    async fn load_secret(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<SecretString, AuthProductError> {
        let lease = self
            .secret_store
            .lease_once(scope, handle)
            .await
            .map_err(|error| {
                if error.is_unknown_secret() {
                    AuthProductError::UnknownOrExpiredFlow
                } else {
                    AuthProductError::BackendUnavailable
                }
            })?;
        self.secret_store
            .consume(scope, lease.id)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)
    }
}

impl fmt::Debug for OAuthDcrProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthDcrProvider")
            .field("provider_id", &self.spec.provider_id)
            .field("callback_origin", &self.callback_origin)
            .field("client_name", &self.client_name)
            .field("scopes", &self.scopes)
            .field("acknowledged_settings", &self.primitive_settings.is_some())
            .finish()
    }
}

#[async_trait]
impl OAuthClientMaterialSource for OAuthDcrProvider {
    async fn exchange_material(
        &self,
        scope: &ResourceScope,
        flow_id: AuthFlowId,
    ) -> Result<OAuthClientMaterial, AuthProductError> {
        self.load_flow_client_material(scope, flow_id).await
    }

    async fn refresh_material(
        &self,
        scope: &ResourceScope,
        refresh_secret: &SecretHandle,
    ) -> Result<OAuthClientMaterial, AuthProductError> {
        self.load_refresh_client_material(scope, refresh_secret)
            .await
    }

    async fn bind_refresh_material(
        &self,
        scope: &ResourceScope,
        flow_id: AuthFlowId,
        refresh_secret: &SecretHandle,
    ) -> Result<(), AuthProductError> {
        OAuthDcrProvider::bind_refresh_material(self, scope, flow_id, refresh_secret).await
    }

    async fn bind_rotated_refresh_material(
        &self,
        scope: &ResourceScope,
        previous: &SecretHandle,
        replacement: &SecretHandle,
    ) -> Result<(), AuthProductError> {
        let material = self
            .load_material(scope, &refresh_secret_handle(&self.spec, previous)?)
            .await?;
        self.put_material(
            scope,
            refresh_secret_handle(&self.spec, replacement)?,
            &material,
        )
        .await
    }

    async fn cleanup_refresh_material(
        &self,
        scope: &ResourceScope,
        refresh_secret: &SecretHandle,
    ) -> Result<(), AuthProductError> {
        self.secret_store
            .delete(scope, &refresh_secret_handle(&self.spec, refresh_secret)?)
            .await
            .map(|_| ())
            .map_err(|_| AuthProductError::BackendUnavailable)
    }

    async fn cleanup_exchange_material(
        &self,
        scope: &ResourceScope,
        flow_id: AuthFlowId,
    ) -> Result<(), AuthProductError> {
        self.cleanup_flow_material(scope, flow_id).await
    }
}

#[derive(Clone, Default)]
pub(crate) struct OAuthDcrProviderRegistry {
    providers: BTreeMap<String, Arc<OAuthDcrProvider>>,
}

impl OAuthDcrProviderRegistry {
    pub(crate) fn new(providers: Vec<Arc<OAuthDcrProvider>>) -> Self {
        Self {
            providers: providers
                .into_iter()
                .map(|provider| (provider.spec.provider_id.to_string(), provider))
                .collect(),
        }
    }

    pub(crate) async fn challenge_for_blocked_gate(
        &self,
        request: DcrGateChallengeRequest<'_>,
    ) -> Result<Option<AuthChallengeView>, AuthProductError> {
        let DcrGateChallengeRequest {
            flow_manager,
            flow_source,
            requirements,
            scope,
            owner_user_id,
            run_id,
            gate_ref,
        } = request;
        let [requirement] = requirements else {
            return Ok(None);
        };
        let provider = requirement.provider.as_str();
        let Some(dcr_provider) = self.providers.get(provider) else {
            return Ok(None);
        };
        dcr_provider
            .challenge_for_blocked_gate(
                flow_manager,
                flow_source,
                scope,
                owner_user_id,
                run_id,
                gate_ref,
            )
            .await
            .map(Some)
    }

    #[allow(
        dead_code,
        reason = "used by the webui-v2-beta OAuth callback route through RebornProductAuthServices"
    )]
    pub(crate) async fn pkce_verifier_for_flow(
        &self,
        manager: &Arc<dyn AuthFlowManager>,
        scope: &AuthProductScope,
        provider: &AuthProviderId,
        flow_id: AuthFlowId,
    ) -> Result<Option<SecretString>, AuthProductError> {
        let Some(dcr_provider) = self.providers.get(provider.as_str()) else {
            return Ok(None);
        };
        if manager
            .read_dcr_registration(scope, flow_id)
            .await?
            .is_none()
        {
            let flow = manager
                .get_flow(scope, flow_id)
                .await?
                .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
            let actual = dcr_provider.retain_legacy_flow(manager, flow).await?;
            if brassclaw_auth::is_terminal_status(actual.status) {
                return Ok(None);
            }
        }
        dcr_provider.pkce_verifier_for_flow(scope, flow_id).await
    }

    pub(crate) async fn legacy_definition(
        &self,
        flow: &brassclaw_auth::AuthFlowRecord,
    ) -> Result<brassclaw_auth::DcrLegacyOriginal, AuthProductError> {
        self.providers
            .get(flow.provider.as_str())
            .ok_or(AuthProductError::BackendUnavailable)?
            .legacy_flow_definition(flow)
            .await
    }

    /// Read the protected first definition without discovery, fresh randomness
    /// or provider I/O; its Auth projection may still be missing.
    pub(crate) async fn retained_setup_preparation(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
        provider: &AuthProviderId,
    ) -> Result<Option<brassclaw_auth::DcrRegistrationOriginal>, AuthProductError> {
        let Some(provider) = self.providers.get(provider.as_str()) else {
            return Ok(None);
        };
        provider.retained_preparation_original(scope, flow_id).await
    }

    pub(crate) async fn cleanup_original(
        &self,
        manager: &Arc<dyn AuthFlowManager>,
        state: brassclaw_auth::DcrRegistrationState,
    ) -> Result<brassclaw_auth::AuthFlowRecord, AuthProductError> {
        let provider = self
            .providers
            .get(state.action.original.provider.as_str())
            .ok_or(AuthProductError::BackendUnavailable)?;
        provider.cleanup_original_registration(manager, state).await
    }

    #[allow(
        dead_code,
        reason = "used by the webui-v2-beta extension OAuth route through RebornProductAuthServices"
    )]
    pub(crate) async fn start_setup_flow(
        &self,
        flow_manager: &Arc<dyn AuthFlowManager>,
        request: DcrSetupFlowRequest,
    ) -> Result<Option<brassclaw_auth::AuthFlowRecord>, AuthProductError> {
        let DcrSetupFlowRequest {
            requirement_setup,
            flow_id,
            scope,
            provider,
            account_label,
            provider_scopes,
            update_binding,
            expires_at,
        } = request;
        let Some(dcr_provider) = self.providers.get(provider.as_str()) else {
            return Ok(None);
        };
        dcr_provider
            .start_original_registration(
                flow_manager,
                DcrOriginalStart {
                    requirement_setup,
                    flow_id,
                    scope,
                    account_label,
                    provider_scopes,
                    update_binding,
                    expires_at,
                    continuation: AuthContinuationRef::SetupOnly,
                },
            )
            .await
            .map(Some)
    }
}

impl fmt::Debug for OAuthDcrProviderRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthDcrProviderRegistry")
            .field("providers", &self.providers.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[derive(Debug, Clone)]
struct CachedAuthorizationServerMetadata {
    metadata: AuthorizationServerMetadata,
    stored_at: Instant,
    ttl_secs: u32,
}
impl CachedAuthorizationServerMetadata {
    fn is_current(&self, current_ttl_secs: u32) -> bool {
        let ttl = self.ttl_secs.min(current_ttl_secs);
        ttl != 0 && self.stored_at.elapsed() < Duration::from_secs(u64::from(ttl))
    }
}

pub(crate) struct DcrGateChallengeRequest<'a> {
    pub(crate) flow_manager: &'a Arc<dyn AuthFlowManager>,
    pub(crate) flow_source: &'a Arc<dyn AuthFlowRecordSource>,
    pub(crate) requirements: &'a [RuntimeCredentialAuthRequirement],
    pub(crate) scope: &'a TurnScope,
    pub(crate) owner_user_id: &'a brassclaw_host_api::UserId,
    pub(crate) run_id: TurnRunId,
    pub(crate) gate_ref: &'a AuthGateRef,
}

pub(crate) struct DcrSetupFlowRequest {
    /// Complete usage and previous selection captured before the challenge.
    pub(crate) requirement_setup: Option<Box<brassclaw_auth::CredentialRequirementSetup>>,
    pub(crate) flow_id: AuthFlowId,
    pub(crate) scope: AuthProductScope,
    pub(crate) provider: AuthProviderId,
    pub(crate) account_label: CredentialAccountLabel,
    pub(crate) provider_scopes: Vec<ProviderScope>,
    pub(crate) update_binding: Option<CredentialAccountUpdateBinding>,
    pub(crate) expires_at: brassclaw_auth::Timestamp,
}

#[derive(Debug, Clone)]
pub(crate) struct DcrOAuthCallbackState {
    flow_id: AuthFlowId,
    scope: AuthProductScope,
    provider: AuthProviderId,
    account_label: CredentialAccountLabel,
    requested_scopes: Vec<ProviderScope>,
    nonce: String,
}

#[derive(Serialize, Deserialize)]
struct DcrOAuthCallbackStateWire {
    flow_id: AuthFlowId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    surface: Option<brassclaw_auth::AuthSurface>,
    resource: ResourceScope,
    session_id: Option<brassclaw_auth::AuthSessionId>,
    provider: AuthProviderId,
    account_label: CredentialAccountLabel,
    requested_scopes: Vec<ProviderScope>,
    nonce: String,
}

#[allow(dead_code)] // Used by the optional product-auth route surface; all-features test targets can compile this module without that route.
impl DcrOAuthCallbackState {
    const PREFIX: &'static str = "icd1.";

    pub(crate) fn new(
        flow_id: AuthFlowId,
        scope: AuthProductScope,
        provider: AuthProviderId,
        account_label: CredentialAccountLabel,
        requested_scopes: Vec<ProviderScope>,
    ) -> Self {
        Self {
            flow_id,
            scope,
            provider,
            account_label,
            requested_scopes,
            nonce: brassclaw_common::pkce::generate_code_verifier(),
        }
    }

    pub(crate) fn encode(&self) -> Result<OAuthState, AuthProductError> {
        let wire = DcrOAuthCallbackStateWire {
            flow_id: self.flow_id,
            surface: if self.scope.surface == brassclaw_auth::AuthSurface::Callback {
                None
            } else {
                Some(self.scope.surface)
            },
            resource: self.scope.resource.clone(),
            session_id: self.scope.session_id.clone(),
            provider: self.provider.clone(),
            account_label: self.account_label.clone(),
            requested_scopes: self.requested_scopes.clone(),
            nonce: self.nonce.clone(),
        };
        let payload =
            serde_json::to_vec(&wire).map_err(|_| AuthProductError::BackendUnavailable)?;
        OAuthState::new(format!(
            "{}{}",
            Self::PREFIX,
            URL_SAFE_NO_PAD.encode(payload)
        ))
    }
}

impl DcrOAuthCallbackState {
    pub(crate) fn has_prefix(raw: &str) -> bool {
        raw.starts_with(Self::PREFIX)
    }

    pub(crate) fn flow_id(&self) -> AuthFlowId {
        self.flow_id
    }

    pub(crate) fn scope(&self) -> &AuthProductScope {
        &self.scope
    }

    pub(crate) fn provider(&self) -> &AuthProviderId {
        &self.provider
    }

    pub(crate) fn account_label(&self) -> &CredentialAccountLabel {
        &self.account_label
    }

    pub(crate) fn requested_scopes(&self) -> &[ProviderScope] {
        &self.requested_scopes
    }

    pub(crate) fn decode(raw: &str) -> Result<Self, AuthProductError> {
        let encoded = raw
            .strip_prefix(Self::PREFIX)
            .ok_or(AuthProductError::MalformedCallback)?;
        let payload = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| AuthProductError::MalformedCallback)?;
        let wire: DcrOAuthCallbackStateWire =
            serde_json::from_slice(&payload).map_err(|_| AuthProductError::MalformedCallback)?;
        OAuthState::new(wire.nonce.clone()).map_err(|_| AuthProductError::MalformedCallback)?;
        let mut scope = AuthProductScope::new(
            wire.resource,
            wire.surface
                .unwrap_or(brassclaw_auth::AuthSurface::Callback),
        );
        if let Some(session_id) = wire.session_id {
            scope = scope.with_session_id(session_id);
        }
        Ok(Self {
            flow_id: wire.flow_id,
            scope,
            provider: wire.provider,
            account_label: wire.account_label,
            requested_scopes: wire.requested_scopes,
            nonce: wire.nonce,
        })
    }
}

fn auth_scope_for_blocked_turn(
    scope: &TurnScope,
    owner_user_id: &brassclaw_host_api::UserId,
) -> AuthProductScope {
    AuthProductScope::new(
        ResourceScope {
            tenant_id: scope.tenant_id.clone(),
            user_id: owner_user_id.clone(),
            agent_id: scope.agent_id.clone(),
            project_id: scope.project_id.clone(),
            thread_id: Some(scope.thread_id.clone()),
            invocation_id: InvocationId::new(),
        },
        brassclaw_auth::AuthSurface::Callback,
    )
}

fn challenge_view_from_flow(
    flow: &brassclaw_auth::AuthFlowRecord,
) -> Result<AuthChallengeView, AuthProductError> {
    if flow.status != brassclaw_auth::AuthFlowStatus::AwaitingUser || flow.expires_at <= Utc::now()
    {
        return Err(if flow.status == brassclaw_auth::AuthFlowStatus::Canceled {
            AuthProductError::Canceled
        } else {
            AuthProductError::UnknownOrExpiredFlow
        });
    }
    let Some(AuthChallenge::OAuthUrl {
        authorization_url,
        expires_at,
    }) = &flow.challenge
    else {
        return Err(AuthProductError::BackendUnavailable);
    };
    Ok(AuthChallengeView {
        kind: AuthPromptChallengeKind::OAuthUrl,
        provider: flow.provider.clone(),
        account_label: None,
        authorization_url: Some(authorization_url.clone()),
        expires_at: Some(*expires_at),
    })
}

#[cfg(test)]
pub(crate) use tests::FaultingDcrSecretStore as TestSecretFaultStore;

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_auth::NewAuthFlow;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[cfg(feature = "skills-db")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_rotated_refresh_binding_retains_exact_original_registration() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let store = Arc::new(
            brassclaw_secrets::PgSecretStore::new(
                rig.pool.as_ref().clone(),
                "controlled-dcr-rotation-material-master-key".into(),
                "dcr-rotated-binding",
            )
            .unwrap(),
        );
        let provider = OAuthDcrProvider::new(
            test_config(),
            Arc::new(DcrSetupEgress),
            store,
            Arc::new(TestObligationHandler),
        )
        .unwrap();
        let scope = sample_auth_scope();
        let flow = AuthFlowId::new();
        let material = StoredDcrClientMaterial {
            client_id: "original-registered-client".into(),
            client_secret: Some(SecretString::from("original-client-secret")),
            redirect_uri: "https://app.example/callback".into(),
            token_endpoint: "https://issuer.example/token".into(),
        };
        provider
            .put_secret(
                &scope.resource,
                flow_secret_handle(&provider.spec, flow, "pkce").unwrap(),
                SecretString::from("private-pkce-fixture"),
            )
            .await
            .unwrap();
        provider
            .put_material(
                &scope.resource,
                flow_secret_handle(&provider.spec, flow, "client").unwrap(),
                &material,
            )
            .await
            .unwrap();
        let old = SecretHandle::new("old-refresh-token-artifact").unwrap();
        let replacement = SecretHandle::new("new-refresh-token-artifact").unwrap();
        provider
            .bind_refresh_material(&scope.resource, flow, &old)
            .await
            .unwrap();
        provider
            .bind_rotated_refresh_material(&scope.resource, &old, &replacement)
            .await
            .unwrap();
        let rebound = provider
            .refresh_material(&scope.resource, &replacement)
            .await
            .unwrap();
        assert_eq!(rebound.client_id.as_str(), "original-registered-client");
        assert_eq!(
            rebound.client_secret.unwrap().expose_secret(),
            "original-client-secret"
        );
        assert_eq!(rebound.token_endpoint, "https://issuer.example/token");
        assert_eq!(
            rebound.redirect_uri.as_str(),
            "https://app.example/callback"
        );
        provider
            .cleanup_refresh_material(&scope.resource, &replacement)
            .await
            .unwrap();
        assert!(
            provider
                .refresh_material(&scope.resource, &replacement)
                .await
                .is_err()
        );
        assert!(
            provider
                .refresh_material(&scope.resource, &old)
                .await
                .is_ok()
        );
        let mut wrong = scope.resource.clone();
        wrong.user_id = brassclaw_host_api::UserId::new("another-user").unwrap();
        assert!(
            provider
                .bind_rotated_refresh_material(&wrong, &old, &replacement)
                .await
                .is_err()
        );
        assert!(
            provider
                .refresh_material(&scope.resource, &replacement)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn dcr_provider_creates_blocked_gate_flow_and_stores_pkce_material() {
        let provider = OAuthDcrProvider::new(
            OAuthDcrProviderConfig {
                spec: HostOAuthProviderSpec {
                    provider_id: "notion",
                    capability_id: "brassclaw_auth.notion_oauth",
                    token_endpoint: "https://mcp.notion.com/token",
                    secret_handle_prefix: "notion",
                    resource: Some("https://mcp.notion.com/mcp"),
                    exchange_scope_policy:
                        crate::oauth_provider_client::ExchangeScopePolicy::FallbackToRequested,
                },
                callback_origin: "http://127.0.0.1:3000".to_string(),
                client_name: "Brassclaw".to_string(),
                account_label: CredentialAccountLabel::new("notion").unwrap(),
                scopes: Vec::new(),
            },
            Arc::new(DcrSetupEgress),
            Arc::new(brassclaw_secrets::InMemorySecretStore::new()),
            Arc::new(TestObligationHandler),
        )
        .unwrap();
        let auth = Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
        let flow_manager: Arc<dyn AuthFlowManager> = auth.clone();
        let flow_source: Arc<dyn AuthFlowRecordSource> = auth.clone();
        let scope = TurnScope::new(
            brassclaw_host_api::TenantId::new("tenant").unwrap(),
            Some(brassclaw_host_api::AgentId::new("agent").unwrap()),
            Some(brassclaw_host_api::ProjectId::new("project").unwrap()),
            brassclaw_host_api::ThreadId::new("thread").unwrap(),
        );
        let owner = brassclaw_host_api::UserId::new("user").unwrap();
        let run_id = TurnRunId::new();
        let gate_ref =
            AuthGateRef::new("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()).unwrap();

        let view = provider
            .challenge_for_blocked_gate(
                &flow_manager,
                &flow_source,
                &scope,
                &owner,
                run_id,
                &gate_ref,
            )
            .await
            .unwrap();

        assert!(matches!(view.kind, AuthPromptChallengeKind::OAuthUrl));
        assert_eq!(view.provider.as_str(), "notion");
        let authorization_url = view.authorization_url.unwrap();
        assert!(authorization_url.as_str().contains("client_id=dcr-client"));
        assert!(
            authorization_url
                .as_str()
                .contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A3000")
        );

        let flow = flow_source
            .flow_for_turn_gate(TurnGateAuthFlowQuery {
                owner: AuthFlowOwnerScope {
                    tenant_id: scope.tenant_id.clone(),
                    user_id: owner.clone(),
                    agent_id: scope.agent_id.clone(),
                    project_id: scope.project_id.clone(),
                    thread_id: scope.thread_id.clone(),
                },
                turn_run_ref: TurnRunRef::new(run_id.to_string()).unwrap(),
                gate_ref,
                include_terminal: false,
            })
            .await
            .unwrap()
            .expect("flow");

        let pkce = provider
            .pkce_verifier_for_flow(&flow.scope, flow.id)
            .await
            .unwrap();
        assert!(pkce.is_some());
    }

    #[tokio::test]
    async fn dcr_provider_creates_setup_only_flow_and_stores_pkce_material() {
        let provider = test_provider(Arc::new(DcrSetupEgress));
        let auth = Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
        let flow_manager: Arc<dyn AuthFlowManager> = auth.clone();

        let flow = provider
            .start_original_registration(
                &flow_manager,
                DcrOriginalStart {
                    requirement_setup: None,
                    flow_id: AuthFlowId::new(),
                    scope: sample_auth_scope(),
                    account_label: CredentialAccountLabel::new("work notion").unwrap(),
                    provider_scopes: vec![],
                    update_binding: None,
                    expires_at: Utc::now() + ChronoDuration::seconds(DCR_FLOW_TTL_SECONDS),
                    continuation: AuthContinuationRef::SetupOnly,
                },
            )
            .await
            .unwrap();

        assert_eq!(flow.provider.as_str(), "notion");
        assert!(matches!(flow.continuation, AuthContinuationRef::SetupOnly));
        let Some(AuthChallenge::OAuthUrl {
            authorization_url, ..
        }) = &flow.challenge
        else {
            panic!("setup flow should render an OAuth URL challenge");
        };
        assert!(authorization_url.as_str().contains("client_id=dcr-client"));
        let parsed = url::Url::parse(authorization_url.as_str()).unwrap();
        let redirect_uri = parsed
            .query_pairs()
            .find_map(|(name, value)| (name == "redirect_uri").then(|| value.into_owned()))
            .expect("redirect uri");
        let redirect = url::Url::parse(&redirect_uri).unwrap();
        assert_eq!(
            redirect
                .query_pairs()
                .find_map(|(name, value)| (name == "account_label").then(|| value.into_owned())),
            Some("work notion".to_string())
        );
        assert!(
            authorization_url
                .as_str()
                .contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A3000")
        );
        let pkce = provider
            .pkce_verifier_for_flow(&flow.scope, flow.id)
            .await
            .unwrap();
        assert!(pkce.is_some());
    }

    #[tokio::test]
    async fn dcr_provider_setup_flow_rejects_scope_mismatch() {
        let provider = test_provider(Arc::new(DcrSetupEgress));
        let auth = Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
        let flow_manager: Arc<dyn AuthFlowManager> = auth.clone();

        let error = provider
            .start_original_registration(
                &flow_manager,
                DcrOriginalStart {
                    requirement_setup: None,
                    flow_id: AuthFlowId::new(),
                    scope: sample_auth_scope(),
                    account_label: CredentialAccountLabel::new("work notion").unwrap(),
                    provider_scopes: vec![ProviderScope::new("read").unwrap()],
                    update_binding: None,
                    expires_at: Utc::now() + ChronoDuration::seconds(DCR_FLOW_TTL_SECONDS),
                    continuation: AuthContinuationRef::SetupOnly,
                },
            )
            .await
            .expect_err("scope mismatch should be rejected before registration");

        assert_eq!(
            error.code(),
            brassclaw_auth::AuthErrorCode::BackendUnavailable
        );
        assert!(
            auth.flows_for_owner(sample_flow_owner())
                .await
                .unwrap()
                .is_empty(),
            "scope mismatch must not create a setup flow"
        );
    }

    #[test]
    fn dcr_oauth_callback_state_round_trips_callback_fields() {
        let scope = sample_auth_scope();
        let provider = AuthProviderId::new("notion").unwrap();
        let account_label = CredentialAccountLabel::new("work notion").unwrap();
        let requested_scopes = vec![ProviderScope::new("read").unwrap()];
        let state = DcrOAuthCallbackState::new(
            AuthFlowId::new(),
            scope.clone(),
            provider.clone(),
            account_label.clone(),
            requested_scopes.clone(),
        );

        let encoded = state.encode().expect("encoded DCR callback state");
        let decoded = DcrOAuthCallbackState::decode(encoded.as_str())
            .expect("encoded DCR callback state should decode");

        assert!(DcrOAuthCallbackState::has_prefix(encoded.as_str()));
        assert_eq!(decoded.flow_id(), state.flow_id());
        assert_eq!(decoded.scope(), &scope);
        assert_eq!(decoded.provider(), &provider);
        assert_eq!(decoded.account_label(), &account_label);
        assert_eq!(decoded.requested_scopes(), requested_scopes.as_slice());
    }

    #[test]
    fn dcr_oauth_callback_state_rejects_missing_prefix_or_corrupt_payload() {
        let missing_prefix = DcrOAuthCallbackState::decode("not-dcr-state")
            .expect_err("missing DCR prefix should fail");
        let corrupt_payload =
            DcrOAuthCallbackState::decode("icd1.not-base64").expect_err("corrupt payload fails");

        assert_eq!(
            missing_prefix.code(),
            brassclaw_auth::AuthErrorCode::MalformedCallback
        );
        assert_eq!(
            corrupt_payload.code(),
            brassclaw_auth::AuthErrorCode::MalformedCallback
        );
    }

    #[tokio::test]
    async fn discover_authorization_server_empty_authorization_servers_returns_backend_unavailable()
    {
        let provider = test_provider(Arc::new(DcrDiscoveryEgress::empty_authorization_servers()));
        let error = provider
            .discover_authorization_server(&sample_resource_scope())
            .await
            .expect_err("empty authorization_servers must fail");

        assert_eq!(
            error.code(),
            brassclaw_auth::AuthErrorCode::BackendUnavailable
        );
    }

    #[tokio::test]
    async fn discover_authorization_server_falls_back_to_issuer_url_when_resource_metadata_fails() {
        let provider = test_provider(Arc::new(DcrDiscoveryEgress::resource_metadata_fails()));

        let metadata = provider
            .discover_authorization_server(&sample_resource_scope())
            .await
            .expect("issuer fallback metadata");

        assert_eq!(
            metadata.registration_endpoint,
            "https://mcp.notion.com/register"
        );
    }

    #[tokio::test]
    async fn discover_authorization_server_empty_registration_endpoint_returns_backend_unavailable()
    {
        let provider = test_provider(Arc::new(DcrDiscoveryEgress::empty_registration_endpoint()));

        let error = provider
            .discover_authorization_server(&sample_resource_scope())
            .await
            .expect_err("empty registration endpoint must fail");

        assert_eq!(
            error.code(),
            brassclaw_auth::AuthErrorCode::BackendUnavailable
        );
    }

    #[tokio::test]
    async fn original_dcr_retains_every_known_or_unknown_material_write_without_compensation_or_replay()
     {
        for failure_at in 2..=8 {
            for after_commit in [false, true] {
                let secret_store =
                    Arc::new(FaultingDcrSecretStore::new_at(failure_at, after_commit));
                let egress = Arc::new(RecordingDcrSetupEgress::new());
                let provider = OAuthDcrProvider::new(
                    test_config(),
                    egress.clone(),
                    secret_store.clone(),
                    Arc::new(TestObligationHandler),
                )
                .unwrap();
                let auth = Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
                let manager: Arc<dyn AuthFlowManager> = auth;
                let scope = sample_auth_scope();
                let flow_id = AuthFlowId::new();
                let label = CredentialAccountLabel::new("original notion").unwrap();
                let expiry = Utc::now() + ChronoDuration::seconds(DCR_FLOW_TTL_SECONDS);
                let request = DcrOriginalStart {
                    requirement_setup: None,
                    flow_id,
                    scope: scope.clone(),
                    account_label: label,
                    provider_scopes: vec![],
                    continuation: AuthContinuationRef::SetupOnly,
                    update_binding: None,
                    expires_at: expiry,
                };
                let first = provider
                    .start_original_registration(&manager, request.clone())
                    .await;
                let before = manager
                    .read_dcr_registration(&scope, flow_id)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(before.flow.id, flow_id);
                assert_eq!(before.flow.expires_at, expiry);
                assert!(!secret_store.put_handles().is_empty());
                assert!(secret_store.deleted_handles().is_empty());
                let resumed = provider
                    .start_original_registration(&manager, request)
                    .await;
                if failure_at == 3 && !after_commit {
                    assert!(first.is_err());
                    assert!(resumed.is_err());
                    assert!(matches!(
                        before.action.outcome,
                        brassclaw_auth::DcrRegistrationOutcome::Submitted { .. }
                    ));
                    assert_eq!(before.flow.status, brassclaw_auth::AuthFlowStatus::Pending);
                    assert!(before.flow.challenge.is_none());
                } else {
                    let flow = resumed.unwrap();
                    assert_eq!(flow.status, brassclaw_auth::AuthFlowStatus::AwaitingUser);
                    assert_eq!(flow.id, flow_id);
                    assert_eq!(flow.expires_at, expiry);
                    assert!(
                        provider
                            .pkce_verifier_for_flow(&scope, flow_id)
                            .await
                            .unwrap()
                            .is_some()
                    );
                }
                let sent = egress.requests();
                assert_eq!(
                    sent.iter()
                        .filter(|(method, _)| *method == NetworkMethod::Post)
                        .count(),
                    1,
                    "one issuer registration, failure_at={failure_at}, after_commit={after_commit}"
                );
                assert!(
                    !sent
                        .iter()
                        .any(|(method, _)| *method == NetworkMethod::Delete)
                );
                assert!(secret_store.deleted_handles().is_empty());
                assert_eq!(
                    manager
                        .read_dcr_registration(&scope, flow_id)
                        .await
                        .unwrap()
                        .unwrap()
                        .action
                        .original,
                    before.action.original
                );
            }
        }
    }

    #[tokio::test]
    async fn blocked_gate_reuses_one_original_after_pre_dispatch_material_failure() {
        let store = Arc::new(FaultingDcrSecretStore::new_at(2, false));
        let egress = Arc::new(RecordingDcrSetupEgress::new());
        let provider = OAuthDcrProvider::new(
            test_config(),
            egress.clone(),
            store.clone(),
            Arc::new(TestObligationHandler),
        )
        .unwrap();
        let auth = Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
        let manager: Arc<dyn AuthFlowManager> = auth.clone();
        let source: Arc<dyn AuthFlowRecordSource> = auth.clone();
        let scope = sample_turn_scope();
        let owner = brassclaw_host_api::UserId::new("user").unwrap();
        let run = TurnRunId::new();
        let gate = AuthGateRef::new("original-dcr-gate").unwrap();
        assert!(
            provider
                .challenge_for_blocked_gate(&manager, &source, &scope, &owner, run, &gate)
                .await
                .is_err()
        );
        assert!(
            !egress
                .requests()
                .iter()
                .any(|(method, _)| *method == NetworkMethod::Post)
        );
        let query = TurnGateAuthFlowQuery {
            owner: sample_flow_owner(),
            turn_run_ref: TurnRunRef::new(run.to_string()).unwrap(),
            gate_ref: gate.clone(),
            include_terminal: true,
        };
        let before = source
            .flow_for_turn_gate(query.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(before.status, brassclaw_auth::AuthFlowStatus::Pending);
        assert!(
            provider
                .challenge_for_blocked_gate(&manager, &source, &scope, &owner, run, &gate)
                .await
                .is_ok()
        );
        let after = source.flow_for_turn_gate(query).await.unwrap().unwrap();
        assert_eq!(before.id, after.id);
        assert_eq!(before.scope, after.scope);
        assert_eq!(before.expires_at, after.expires_at);
        assert_eq!(
            egress
                .requests()
                .iter()
                .filter(|(method, _)| *method == NetworkMethod::Post)
                .count(),
            1
        );
        assert!(store.deleted_handles().is_empty());
    }

    #[tokio::test]
    async fn unavailable_original_flow_owner_prevents_registration_before_any_issuer_effect() {
        let egress = Arc::new(RecordingDcrSetupEgress::new());
        let provider = OAuthDcrProvider::new(
            test_config(),
            egress.clone(),
            Arc::new(brassclaw_secrets::InMemorySecretStore::new()),
            Arc::new(TestObligationHandler),
        )
        .unwrap();
        let manager: Arc<dyn AuthFlowManager> = Arc::new(BackendUnavailableFlowManager);
        let source: Arc<dyn AuthFlowRecordSource> =
            Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
        let result = provider
            .challenge_for_blocked_gate(
                &manager,
                &source,
                &sample_turn_scope(),
                &brassclaw_host_api::UserId::new("user").unwrap(),
                TurnRunId::new(),
                &AuthGateRef::new("unavailable-original").unwrap(),
            )
            .await;
        assert_eq!(result.unwrap_err(), AuthProductError::BackendUnavailable);
        assert!(
            !egress
                .requests()
                .iter()
                .any(|(method, _)| matches!(method, NetworkMethod::Post | NetworkMethod::Delete))
        );
    }

    #[tokio::test]
    async fn pkce_verifier_for_flow_returns_none_when_secret_not_found() {
        let provider = test_provider(Arc::new(TestEgress));

        let pkce = provider
            .pkce_verifier_for_flow(&sample_auth_scope(), AuthFlowId::new())
            .await
            .unwrap();

        assert!(pkce.is_none());
    }

    #[test]
    fn callback_redirect_uri_carries_existing_callback_query_fields() {
        let provider = OAuthDcrProvider::new(
            OAuthDcrProviderConfig {
                spec: HostOAuthProviderSpec {
                    provider_id: "notion",
                    capability_id: "brassclaw_auth.notion_oauth",
                    token_endpoint: "https://mcp.notion.com/token",
                    secret_handle_prefix: "notion",
                    resource: Some("https://mcp.notion.com/mcp"),
                    exchange_scope_policy:
                        crate::oauth_provider_client::ExchangeScopePolicy::FallbackToRequested,
                },
                callback_origin: "http://127.0.0.1:3000".to_string(),
                client_name: "Brassclaw".to_string(),
                account_label: CredentialAccountLabel::new("notion").unwrap(),
                scopes: vec![ProviderScope::new("read").unwrap()],
            },
            Arc::new(TestEgress),
            Arc::new(brassclaw_secrets::InMemorySecretStore::new()),
            Arc::new(TestObligationHandler),
        )
        .unwrap();
        let scope = AuthProductScope::new(
            ResourceScope {
                tenant_id: brassclaw_host_api::TenantId::new("tenant").unwrap(),
                user_id: brassclaw_host_api::UserId::new("user").unwrap(),
                agent_id: None,
                project_id: None,
                thread_id: Some(brassclaw_host_api::ThreadId::new("thread").unwrap()),
                invocation_id: InvocationId::new(),
            },
            brassclaw_auth::AuthSurface::Callback,
        );

        let redirect = provider
            .callback_redirect_uri(
                &scope,
                AuthFlowId::from_uuid(uuid::Uuid::nil()),
                &CredentialAccountLabel::new("notion").unwrap(),
            )
            .unwrap();

        assert!(
            redirect
                .as_str()
                .contains("/api/reborn/product-auth/oauth/callback/")
        );
        assert!(redirect.as_str().contains("provider=notion"));
        assert!(redirect.as_str().contains("account_label=notion"));
        assert!(redirect.as_str().contains("scope=read"));
    }

    fn test_provider(egress: Arc<dyn RuntimeHttpEgress>) -> OAuthDcrProvider {
        OAuthDcrProvider::new(
            test_config(),
            egress,
            Arc::new(brassclaw_secrets::InMemorySecretStore::new()),
            Arc::new(TestObligationHandler),
        )
        .unwrap()
    }

    pub(super) fn test_config() -> OAuthDcrProviderConfig {
        OAuthDcrProviderConfig {
            spec: HostOAuthProviderSpec {
                provider_id: "notion",
                capability_id: "brassclaw_auth.notion_oauth",
                token_endpoint: "https://mcp.notion.com/token",
                secret_handle_prefix: "notion",
                resource: Some("https://mcp.notion.com/mcp"),
                exchange_scope_policy:
                    crate::oauth_provider_client::ExchangeScopePolicy::FallbackToRequested,
            },
            callback_origin: "http://127.0.0.1:3000".to_string(),
            client_name: "Brassclaw".to_string(),
            account_label: CredentialAccountLabel::new("notion").unwrap(),
            scopes: Vec::new(),
        }
    }

    fn sample_resource_scope() -> ResourceScope {
        sample_auth_scope().resource
    }

    pub(super) fn sample_auth_scope() -> AuthProductScope {
        AuthProductScope::new(
            ResourceScope {
                tenant_id: brassclaw_host_api::TenantId::new("tenant").unwrap(),
                user_id: brassclaw_host_api::UserId::new("user").unwrap(),
                agent_id: Some(brassclaw_host_api::AgentId::new("agent").unwrap()),
                project_id: Some(brassclaw_host_api::ProjectId::new("project").unwrap()),
                thread_id: Some(brassclaw_host_api::ThreadId::new("thread").unwrap()),
                invocation_id: InvocationId::new(),
            },
            brassclaw_auth::AuthSurface::Callback,
        )
    }

    fn sample_turn_scope() -> TurnScope {
        TurnScope::new(
            brassclaw_host_api::TenantId::new("tenant").unwrap(),
            Some(brassclaw_host_api::AgentId::new("agent").unwrap()),
            Some(brassclaw_host_api::ProjectId::new("project").unwrap()),
            brassclaw_host_api::ThreadId::new("thread").unwrap(),
        )
    }

    fn sample_flow_owner() -> AuthFlowOwnerScope {
        let scope = sample_auth_scope();
        AuthFlowOwnerScope {
            tenant_id: scope.resource.tenant_id,
            user_id: scope.resource.user_id,
            agent_id: scope.resource.agent_id,
            project_id: scope.resource.project_id,
            thread_id: scope.resource.thread_id.unwrap(),
        }
    }

    #[derive(Debug, Clone, Copy)]
    enum DcrDiscoveryCase {
        EmptyAuthorizationServers,
        ResourceMetadataFails,
        EmptyRegistrationEndpoint,
    }

    #[derive(Debug)]
    struct DcrDiscoveryEgress {
        case: DcrDiscoveryCase,
    }

    impl DcrDiscoveryEgress {
        fn empty_authorization_servers() -> Self {
            Self {
                case: DcrDiscoveryCase::EmptyAuthorizationServers,
            }
        }

        fn resource_metadata_fails() -> Self {
            Self {
                case: DcrDiscoveryCase::ResourceMetadataFails,
            }
        }

        fn empty_registration_endpoint() -> Self {
            Self {
                case: DcrDiscoveryCase::EmptyRegistrationEndpoint,
            }
        }
    }

    #[async_trait]
    impl RuntimeHttpEgress for DcrDiscoveryEgress {
        async fn execute(
            &self,
            request: RuntimeHttpEgressRequest,
        ) -> Result<
            brassclaw_host_api::RuntimeHttpEgressResponse,
            brassclaw_host_api::RuntimeHttpEgressError,
        > {
            let (status, body) = match (self.case, request.url.as_str()) {
                (
                    DcrDiscoveryCase::EmptyAuthorizationServers,
                    "https://mcp.notion.com/mcp/.well-known/oauth-protected-resource",
                ) => (200, br#"{"authorization_servers":[]}"#.to_vec()),
                (
                    DcrDiscoveryCase::ResourceMetadataFails,
                    "https://mcp.notion.com/mcp/.well-known/oauth-protected-resource",
                ) => (500, Vec::new()),
                (
                    DcrDiscoveryCase::ResourceMetadataFails,
                    "https://mcp.notion.com/.well-known/oauth-authorization-server",
                ) => (
                    200,
                    br#"{"authorization_endpoint":"https://mcp.notion.com/authorize","token_endpoint":"https://mcp.notion.com/token","registration_endpoint":"https://mcp.notion.com/register"}"#.to_vec(),
                ),
                (
                    DcrDiscoveryCase::EmptyRegistrationEndpoint,
                    "https://mcp.notion.com/mcp/.well-known/oauth-protected-resource",
                ) => (
                    200,
                    br#"{"authorization_servers":["https://oauth.notion.com"]}"#.to_vec(),
                ),
                (
                    DcrDiscoveryCase::EmptyRegistrationEndpoint,
                    "https://oauth.notion.com/.well-known/oauth-authorization-server",
                ) => (
                    200,
                    br#"{"authorization_endpoint":"https://oauth.notion.com/authorize","token_endpoint":"https://oauth.notion.com/token","registration_endpoint":""}"#.to_vec(),
                ),
                other => panic!("unexpected DCR discovery egress URL: {other:?}"),
            };
            Ok(brassclaw_host_api::RuntimeHttpEgressResponse {
                status,
                headers: Vec::new(),
                request_bytes: request.body.len() as u64,
                response_bytes: body.len() as u64,
                body,
                saved_body: None,
                redaction_applied: false,
            })
        }
    }

    #[derive(Debug)]
    pub(crate) struct FaultingDcrSecretStore {
        inner: brassclaw_secrets::InMemorySecretStore,
        put_count: AtomicUsize,
        failure_at: usize,
        after_commit: bool,
        closure_fault: AtomicUsize,
        put_handles: Mutex<Vec<SecretHandle>>,
        deleted_handles: Mutex<Vec<SecretHandle>>,
    }

    impl FaultingDcrSecretStore {
        pub(crate) fn new_at(failure_at: usize, after_commit: bool) -> Self {
            Self {
                inner: brassclaw_secrets::InMemorySecretStore::new(),
                put_count: AtomicUsize::new(0),
                failure_at,
                after_commit,
                closure_fault: AtomicUsize::new(0),
                put_handles: Mutex::new(Vec::new()),
                deleted_handles: Mutex::new(Vec::new()),
            }
        }
        pub(crate) fn fail_next_closure(&self, after_commit: bool) {
            self.closure_fault
                .store(if after_commit { 2 } else { 1 }, Ordering::SeqCst);
        }
        fn fault() -> brassclaw_secrets::SecretStoreError {
            brassclaw_secrets::SecretStoreError::StoreUnavailable {
                reason: "injected DCR write fault".into(),
            }
        }
        fn before_write(
            &self,
            handle: &SecretHandle,
        ) -> Result<bool, brassclaw_secrets::SecretStoreError> {
            self.put_handles.lock().unwrap().push(handle.clone());
            let fail = self.put_count.fetch_add(1, Ordering::SeqCst) + 1 == self.failure_at;
            if fail && !self.after_commit {
                return Err(Self::fault());
            }
            Ok(fail && self.after_commit)
        }
        fn put_handles(&self) -> Vec<SecretHandle> {
            self.put_handles
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        }

        fn deleted_handles(&self) -> Vec<SecretHandle> {
            self.deleted_handles
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        }
    }

    #[async_trait]
    impl SecretStore for FaultingDcrSecretStore {
        async fn close_identity(
            &self,
            scope: ResourceScope,
            handle: SecretHandle,
            expected: brassclaw_secrets::SecretIdentityClosure,
        ) -> Result<
            Option<brassclaw_secrets::SecretCustodyMetadata>,
            brassclaw_secrets::SecretStoreError,
        > {
            let closure_fault = self.closure_fault.swap(0, Ordering::SeqCst);
            if closure_fault == 1 {
                return Err(Self::fault());
            }
            let fail = self.before_write(&handle)?;
            let actual = self.inner.close_identity(scope, handle, expected).await?;
            if fail || closure_fault == 2 {
                return Err(Self::fault());
            }
            Ok(actual)
        }

        async fn put(
            &self,
            scope: ResourceScope,
            handle: SecretHandle,
            material: SecretMaterial,
        ) -> Result<brassclaw_secrets::SecretMetadata, brassclaw_secrets::SecretStoreError>
        {
            self.put_with_expiry(scope, handle, material, None).await
        }

        async fn put_with_expiry(
            &self,
            scope: ResourceScope,
            handle: SecretHandle,
            material: SecretMaterial,
            expires_at: Option<brassclaw_host_api::Timestamp>,
        ) -> Result<brassclaw_secrets::SecretMetadata, brassclaw_secrets::SecretStoreError>
        {
            let fail = self.before_write(&handle)?;
            let actual = self
                .inner
                .put_with_expiry(scope, handle, material, expires_at)
                .await?;
            if fail {
                return Err(Self::fault());
            }
            Ok(actual)
        }

        async fn record_reference(
            &self,
            scope: &ResourceScope,
            handle: &SecretHandle,
        ) -> Result<Option<String>, brassclaw_secrets::SecretStoreError> {
            self.inner.record_reference(scope, handle).await
        }

        async fn put_if_record_reference(
            &self,
            scope: ResourceScope,
            handle: SecretHandle,
            material: SecretMaterial,
            expected: Option<&str>,
        ) -> Result<Option<brassclaw_secrets::SecretMetadata>, brassclaw_secrets::SecretStoreError>
        {
            let fail = self.before_write(&handle)?;
            let actual = self
                .inner
                .put_if_record_reference(scope, handle, material, expected)
                .await?;
            if fail {
                return Err(Self::fault());
            }
            Ok(actual)
        }

        async fn matches(
            &self,
            scope: &ResourceScope,
            handle: &SecretHandle,
            candidate: &SecretMaterial,
        ) -> Result<bool, brassclaw_secrets::SecretStoreError> {
            self.inner.matches(scope, handle, candidate).await
        }

        async fn put_original(
            &self,
            scope: ResourceScope,
            handle: SecretHandle,
            material: SecretMaterial,
            context: serde_json::Value,
        ) -> Result<
            Option<brassclaw_secrets::SecretCustodyMetadata>,
            brassclaw_secrets::SecretStoreError,
        > {
            let fail = self.before_write(&handle)?;
            let actual = self
                .inner
                .put_original(scope, handle, material, context)
                .await?;
            if fail {
                return Err(Self::fault());
            }
            Ok(actual)
        }

        async fn custody(
            &self,
            scope: &ResourceScope,
            handle: &SecretHandle,
        ) -> Result<
            Option<brassclaw_secrets::SecretCustodyMetadata>,
            brassclaw_secrets::SecretStoreError,
        > {
            self.inner.custody(scope, handle).await
        }

        async fn put_retained_from_original(
            &self,
            scope: ResourceScope,
            handle: SecretHandle,
            source: brassclaw_secrets::SecretCustodySource,
        ) -> Result<
            Option<brassclaw_secrets::SecretCustodyMetadata>,
            brassclaw_secrets::SecretStoreError,
        > {
            let fail = self.before_write(&handle)?;
            let actual = self
                .inner
                .put_retained_from_original(scope, handle, source)
                .await?;
            if fail {
                return Err(Self::fault());
            }
            Ok(actual)
        }

        async fn metadata(
            &self,
            scope: &ResourceScope,
            handle: &SecretHandle,
        ) -> Result<Option<brassclaw_secrets::SecretMetadata>, brassclaw_secrets::SecretStoreError>
        {
            self.inner.metadata(scope, handle).await
        }

        async fn delete(
            &self,
            scope: &ResourceScope,
            handle: &SecretHandle,
        ) -> Result<bool, brassclaw_secrets::SecretStoreError> {
            self.deleted_handles
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(handle.clone());
            self.inner.delete(scope, handle).await
        }

        async fn lease_once(
            &self,
            scope: &ResourceScope,
            handle: &SecretHandle,
        ) -> Result<brassclaw_secrets::SecretLease, brassclaw_secrets::SecretStoreError> {
            self.inner.lease_once(scope, handle).await
        }

        async fn lease_once_if_record_reference(
            &self,
            scope: &ResourceScope,
            handle: &SecretHandle,
            expected: &str,
        ) -> Result<Option<brassclaw_secrets::SecretLease>, brassclaw_secrets::SecretStoreError>
        {
            self.inner
                .lease_once_if_record_reference(scope, handle, expected)
                .await
        }

        async fn consume(
            &self,
            scope: &ResourceScope,
            lease_id: brassclaw_secrets::SecretLeaseId,
        ) -> Result<SecretMaterial, brassclaw_secrets::SecretStoreError> {
            self.inner.consume(scope, lease_id).await
        }

        async fn revoke(
            &self,
            scope: &ResourceScope,
            lease_id: brassclaw_secrets::SecretLeaseId,
        ) -> Result<brassclaw_secrets::SecretLease, brassclaw_secrets::SecretStoreError> {
            self.inner.revoke(scope, lease_id).await
        }

        async fn leases_for_scope(
            &self,
            scope: &ResourceScope,
        ) -> Result<Vec<brassclaw_secrets::SecretLease>, brassclaw_secrets::SecretStoreError>
        {
            self.inner.leases_for_scope(scope).await
        }
    }

    #[derive(Debug)]
    pub(super) struct DcrSetupEgress;

    #[async_trait]
    impl RuntimeHttpEgress for DcrSetupEgress {
        async fn execute(
            &self,
            request: RuntimeHttpEgressRequest,
        ) -> Result<
            brassclaw_host_api::RuntimeHttpEgressResponse,
            brassclaw_host_api::RuntimeHttpEgressError,
        > {
            let body = match request.url.as_str() {
                "https://mcp.notion.com/mcp/.well-known/oauth-protected-resource" => {
                    br#"{"authorization_servers":["https://oauth.notion.com"]}"#.to_vec()
                }
                "https://oauth.notion.com/.well-known/oauth-authorization-server" => {
                    br#"{"authorization_endpoint":"https://oauth.notion.com/authorize","token_endpoint":"https://oauth.notion.com/token","registration_endpoint":"https://oauth.notion.com/register"}"#.to_vec()
                }
                "https://oauth.notion.com/register" => br#"{"client_id":"dcr-client","registration_client_uri":"https://oauth.notion.com/register/dcr-client","registration_access_token":"registration-token"}"#.to_vec(),
                "https://oauth.notion.com/register/dcr-client"
                    if request.method == NetworkMethod::Delete =>
                {
                    br#"{}"#.to_vec()
                }
                other => panic!("unexpected DCR egress URL: {other}"),
            };
            Ok(brassclaw_host_api::RuntimeHttpEgressResponse {
                status: 200,
                headers: Vec::new(),
                request_bytes: request.body.len() as u64,
                response_bytes: body.len() as u64,
                body,
                saved_body: None,
                redaction_applied: false,
            })
        }
    }

    #[derive(Debug)]
    struct RecordingDcrSetupEgress {
        requests: Mutex<Vec<(NetworkMethod, String)>>,
    }

    impl RecordingDcrSetupEgress {
        fn new() -> Self {
            Self {
                requests: Mutex::new(Vec::new()),
            }
        }

        fn requests(&self) -> Vec<(NetworkMethod, String)> {
            self.requests
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        }
    }

    #[async_trait]
    impl RuntimeHttpEgress for RecordingDcrSetupEgress {
        async fn execute(
            &self,
            request: RuntimeHttpEgressRequest,
        ) -> Result<
            brassclaw_host_api::RuntimeHttpEgressResponse,
            brassclaw_host_api::RuntimeHttpEgressError,
        > {
            self.requests
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push((request.method, request.url.clone()));
            DcrSetupEgress.execute(request).await
        }
    }

    #[derive(Debug)]
    struct BackendUnavailableFlowManager;

    #[async_trait]
    impl AuthFlowManager for BackendUnavailableFlowManager {
        async fn claim_dcr_cleanup(
            &self,
            _prepared: brassclaw_auth::DcrCleanupPrepared,
        ) -> Result<brassclaw_auth::DcrCleanupAdmission, brassclaw_auth::AuthProductError> {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn record_dcr_cleanup(
            &self,
            _scope: &brassclaw_auth::AuthProductScope,
            _completion: brassclaw_auth::DcrCleanupCompletion,
        ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn update_dcr_retirement(
            &self,
            _original: &brassclaw_auth::DcrRegistrationOriginal,
            _update: brassclaw_auth::DcrRetirementUpdate,
        ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn retain_legacy_dcr(
            &self,
            _selection: brassclaw_auth::DcrLegacySelection,
        ) -> Result<brassclaw_auth::DcrLegacyState, brassclaw_auth::AuthProductError> {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn mark_dcr_cleanup_unavailable(
            &self,
            _original: &brassclaw_auth::DcrRegistrationOriginal,
            _issue: brassclaw_auth::DcrCleanupUnavailable,
        ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn prepare_oauth_setup(
            &self,
            _original: brassclaw_auth::OAuthSetupOriginal,
            _retirement: Option<brassclaw_auth::OAuthSetupRetirement>,
        ) -> Result<brassclaw_auth::OAuthSetupState, brassclaw_auth::AuthProductError> {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn read_oauth_setup(
            &self,
            _scope: &brassclaw_auth::AuthProductScope,
            _flow_id: brassclaw_auth::AuthFlowId,
        ) -> Result<Option<brassclaw_auth::OAuthSetupState>, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn prepare_dcr_registration(
            &self,
            _original: brassclaw_auth::DcrRegistrationOriginal,
        ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn retire_dcr_preparation(
            &self,
            _original: brassclaw_auth::DcrRegistrationOriginal,
            _reason: brassclaw_auth::DcrCleanupReason,
        ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn read_dcr_registration(
            &self,
            _scope: &brassclaw_auth::AuthProductScope,
            _id: brassclaw_auth::AuthFlowId,
        ) -> Result<Option<brassclaw_auth::DcrRegistrationState>, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn claim_dcr_registration(
            &self,
            _original: &brassclaw_auth::DcrRegistrationOriginal,
        ) -> Result<brassclaw_auth::DcrRegistrationAdmission, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn record_dcr_registration(
            &self,
            _scope: &brassclaw_auth::AuthProductScope,
            _completion: brassclaw_auth::DcrRegistrationCompletion,
        ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn publish_dcr_challenge(
            &self,
            _original: &brassclaw_auth::DcrRegistrationOriginal,
            _challenge: brassclaw_auth::DcrRegistrationChallenge,
        ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn create_flow(
            &self,
            _request: NewAuthFlow,
        ) -> Result<brassclaw_auth::AuthFlowRecord, AuthProductError> {
            Err(AuthProductError::BackendUnavailable)
        }

        async fn get_flow(
            &self,
            _scope: &AuthProductScope,
            _flow_id: AuthFlowId,
        ) -> Result<Option<brassclaw_auth::AuthFlowRecord>, AuthProductError> {
            unreachable!("create-flow failure test does not read flows")
        }

        async fn claim_oauth_callback(
            &self,
            _scope: &AuthProductScope,
            _request: brassclaw_auth::OAuthCallbackClaimRequest,
        ) -> Result<brassclaw_auth::OAuthCallbackAdmission, AuthProductError> {
            unreachable!("create-flow failure test does not claim callbacks")
        }

        async fn read_oauth_callback_original(
            &self,
            _scope: &AuthProductScope,
            _reference: &brassclaw_auth::OAuthCallbackReference,
        ) -> Result<Option<brassclaw_auth::OAuthCallbackState>, AuthProductError> {
            unreachable!("create-flow failure test does not use callback custody")
        }

        async fn reconcile_oauth_callback(
            &self,
            _scope: &AuthProductScope,
            _reference: &brassclaw_auth::OAuthCallbackReference,
        ) -> Result<brassclaw_auth::OAuthCallbackState, AuthProductError> {
            unreachable!("create-flow failure test does not use callback custody")
        }

        async fn complete_oauth_callback(
            &self,
            _scope: &AuthProductScope,
            _input: brassclaw_auth::OAuthCallbackInput,
        ) -> Result<brassclaw_auth::AuthFlowRecord, AuthProductError> {
            unreachable!("create-flow failure test does not complete callbacks")
        }

        async fn complete_credential_selection(
            &self,
            _scope: &AuthProductScope,
            _input: brassclaw_auth::CredentialSelectionInput,
        ) -> Result<brassclaw_auth::AuthFlowRecord, AuthProductError> {
            unreachable!("create-flow failure test does not complete selection")
        }

        async fn complete_manual_token(
            &self,
            _scope: &AuthProductScope,
            _input: brassclaw_auth::ManualTokenCompletionInput,
        ) -> Result<brassclaw_auth::AuthFlowRecord, AuthProductError> {
            unreachable!("create-flow failure test does not complete manual tokens")
        }

        async fn cancel_manual_token(
            &self,
            _scope: &AuthProductScope,
            _interaction_id: brassclaw_auth::AuthInteractionId,
        ) -> Result<Option<brassclaw_auth::AuthFlowRecord>, AuthProductError> {
            unreachable!("create-flow failure test does not cancel manual tokens")
        }

        async fn fail_oauth_callback(
            &self,
            _scope: &AuthProductScope,
            _input: brassclaw_auth::OAuthCallbackFailureInput,
        ) -> Result<brassclaw_auth::AuthFlowRecord, AuthProductError> {
            unreachable!("create-flow failure test does not fail callbacks")
        }

        async fn mark_continuation_dispatched(
            &self,
            _scope: &AuthProductScope,
            _flow_id: AuthFlowId,
            _emitted_at: brassclaw_auth::Timestamp,
        ) -> Result<brassclaw_auth::AuthFlowRecord, AuthProductError> {
            unreachable!("create-flow failure test does not mark continuations")
        }

        async fn cancel_flow(
            &self,
            _scope: &AuthProductScope,
            _flow_id: AuthFlowId,
        ) -> Result<brassclaw_auth::AuthFlowRecord, AuthProductError> {
            unreachable!("create-flow failure test does not cancel flows")
        }
    }

    #[derive(Debug)]
    struct TestEgress;

    #[async_trait]
    impl RuntimeHttpEgress for TestEgress {
        async fn execute(
            &self,
            _request: RuntimeHttpEgressRequest,
        ) -> Result<
            brassclaw_host_api::RuntimeHttpEgressResponse,
            brassclaw_host_api::RuntimeHttpEgressError,
        > {
            panic!("test egress should not execute")
        }
    }

    #[derive(Debug)]
    pub(super) struct TestObligationHandler;

    #[async_trait]
    impl CapabilityObligationHandler for TestObligationHandler {
        async fn satisfy(
            &self,
            _request: brassclaw_capabilities::CapabilityObligationRequest<'_>,
        ) -> Result<(), brassclaw_capabilities::CapabilityObligationError> {
            Ok(())
        }
    }
}
