use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use brassclaw_auth::{
    AuthChallenge, AuthContinuationRef, AuthFlowId, AuthFlowManager, AuthFlowOwnerScope,
    AuthFlowRecord, AuthFlowRecordSource, AuthGateRef, AuthProductError, AuthProductScope,
    AuthProviderId, AuthSurface, CredentialAccountLabel, GoogleOAuthCallbackState,
    PkceVerifierSecret, ProviderScope, TurnGateAuthFlowQuery, TurnRunRef,
    build_google_authorization_url, opaque_state_hash, pkce_s256_challenge, pkce_verifier_hash,
};
use brassclaw_host_api::{
    InvocationId, ResourceScope, RuntimeCredentialAuthRequirement, SecretHandle,
};
use brassclaw_product_adapters::AuthPromptChallengeKind;
use brassclaw_secrets::SecretStore;
use brassclaw_turns::{TurnRunId, TurnScope};
#[cfg(test)]
use chrono::Duration as ChronoDuration;
use chrono::Utc;
use secrecy::SecretString;

use crate::auth_prompt::AuthChallengeView;
use crate::input::OAuthClientConfig;

#[path = "oauth_gate_client_material.rs"]
mod client_material;
#[path = "oauth_gate_original.rs"]
mod original;
use crate::oauth_provider_client::OAuthSetupDeadline;
pub(crate) use original::GoogleSetupRequest;
#[cfg(all(test, feature = "postgres"))]
pub(crate) use original::tests::verify_native_private_runtime;

pub(crate) fn owned_google_refresh_material_handles(
    refresh: &SecretHandle,
) -> Result<Vec<SecretHandle>, AuthProductError> {
    client_material::owned_refresh_material_handles(refresh)
}

#[derive(Clone)]
pub(crate) struct GoogleOAuthGateProviderRegistry {
    providers: BTreeMap<String, Arc<GoogleOAuthGateProvider>>,
}

impl GoogleOAuthGateProviderRegistry {
    pub(crate) fn new(providers: Vec<Arc<GoogleOAuthGateProvider>>) -> Self {
        Self {
            providers: providers
                .into_iter()
                .map(|provider| (provider.provider_id().to_string(), provider))
                .collect(),
        }
    }

    pub(crate) async fn challenge_for_blocked_gate(
        &self,
        request: OAuthGateChallengeRequest<'_>,
    ) -> Result<Option<AuthChallengeView>, AuthProductError> {
        for requirement in request.requirements {
            let Some(provider) = self.providers.get(requirement.provider.as_str()) else {
                continue;
            };
            return provider
                .challenge_for_blocked_gate(request, requirement)
                .await
                .map(Some);
        }
        Ok(None)
    }

    pub(crate) async fn pkce_verifier_for_flow(
        &self,
        flow_manager: &Arc<dyn AuthFlowManager>,
        scope: &AuthProductScope,
        provider: &AuthProviderId,
        flow_id: AuthFlowId,
    ) -> Result<Option<SecretString>, AuthProductError> {
        let Some(provider) = self.providers.get(provider.as_str()) else {
            return Ok(None);
        };
        provider
            .pkce_verifier_for_flow(flow_manager, scope, flow_id)
            .await
    }

    pub(crate) fn google_provider(
        &self,
    ) -> Result<&Arc<GoogleOAuthGateProvider>, AuthProductError> {
        self.providers
            .get(brassclaw_auth::GOOGLE_PROVIDER_ID)
            .ok_or(AuthProductError::BackendUnavailable)
    }
}

impl fmt::Debug for GoogleOAuthGateProviderRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GoogleOAuthGateProviderRegistry")
            .field("providers", &self.providers.keys().collect::<Vec<_>>())
            .finish()
    }
}

pub(crate) struct OAuthGateChallengeRequest<'a> {
    pub(crate) flow_manager: &'a Arc<dyn AuthFlowManager>,
    pub(crate) flow_source: &'a Arc<dyn AuthFlowRecordSource>,
    pub(crate) requirements: &'a [RuntimeCredentialAuthRequirement],
    pub(crate) scope: &'a TurnScope,
    pub(crate) owner_user_id: &'a brassclaw_host_api::UserId,
    pub(crate) run_id: TurnRunId,
    pub(crate) gate_ref: &'a AuthGateRef,
}

#[derive(Clone)]
pub(crate) struct GoogleOAuthGateProvider {
    client: OAuthClientConfig,
    secret_store: Arc<dyn SecretStore>,
    primitive_settings: Option<Arc<dyn brassclaw_host_runtime::HostPrimitiveSettingsSource>>,
}

impl GoogleOAuthGateProvider {
    pub(crate) fn new(client: OAuthClientConfig, secret_store: Arc<dyn SecretStore>) -> Self {
        Self {
            client,
            secret_store,
            primitive_settings: None,
        }
    }

    pub(crate) fn with_primitive_settings_source(
        mut self,
        source: Option<Arc<dyn brassclaw_host_runtime::HostPrimitiveSettingsSource>>,
    ) -> Self {
        self.primitive_settings = source;
        self
    }

    pub(crate) fn verify_route_configuration(
        &self,
        target: &brassclaw_auth::GoogleOAuthRouteConfig,
    ) -> Result<(), AuthProductError> {
        if target.client_id() != &self.client.client_id
            || target.redirect_uri() != &self.client.redirect_uri
            || target.hosted_domain_hint() != self.client.hosted_domain_hint.as_deref()
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }

    fn provider_id(&self) -> &'static str {
        brassclaw_auth::GOOGLE_PROVIDER_ID
    }

    async fn challenge_for_blocked_gate(
        &self,
        request: OAuthGateChallengeRequest<'_>,
        requirement: &RuntimeCredentialAuthRequirement,
    ) -> Result<AuthChallengeView, AuthProductError> {
        let mut auth_scope = auth_scope_for_blocked_turn(request.scope, request.owner_user_id);
        let turn_run_ref = TurnRunRef::new(request.run_id.to_string())?;
        let mut query =
            turn_gate_query(&auth_scope, request.scope, &turn_run_ref, request.gate_ref);
        query.include_terminal = true;
        let mut prior = request
            .flow_source
            .flows_for_owner(query.owner.clone())
            .await?
            .into_iter()
            .filter(|flow| {
                brassclaw_auth::flow_matches_turn_gate_query(flow, &query)
                    && flow.provider.as_str() == self.provider_id()
            })
            .collect::<Vec<_>>();
        prior.sort_by_key(|flow| flow.id);
        let mut active = prior
            .iter()
            .filter(|flow| !brassclaw_auth::is_terminal_status(flow.status));
        let existing = active.next();
        if active.next().is_some() {
            return Err(AuthProductError::BackendConflict);
        }
        let scopes = provider_scopes(&requirement.provider_scopes)?;
        if let Some(existing) = existing {
            if existing.expires_at > Utc::now() {
                let original = request
                    .flow_manager
                    .read_oauth_setup(&existing.scope, existing.id)
                    .await?
                    .ok_or(AuthProductError::CredentialMissing)?;
                if original.original.definition.requested_scopes != scopes {
                    return Err(AuthProductError::BackendConflict);
                }
                return challenge_view_from_flow(&original.flow);
            }
            // Preserve the terminal predecessor. The next identity captures
            // the complete sorted predecessor set, without assuming wall-clock
            // order or choosing an arbitrary old UUID as the latest flow.
            request
                .flow_manager
                .cancel_flow(&existing.scope, existing.id)
                .await?;
        }
        let id = stable_gate_identity(
            &auth_scope,
            request.run_id,
            request.gate_ref,
            &prior.iter().map(|flow| flow.id).collect::<Vec<_>>(),
        )?;
        let flow_id = AuthFlowId::from_uuid(id);
        auth_scope.resource.invocation_id = InvocationId::from_uuid(id);
        let original = self
            .prepare_setup_original(GoogleSetupRequest {
                scope: auth_scope,
                flow_id,
                account_label: CredentialAccountLabel::new("google")?,
                requested_scopes: scopes,
                continuation: AuthContinuationRef::TurnGateResume {
                    turn_run_ref,
                    gate_ref: request.gate_ref.clone(),
                },
                requirement_setup: None,
                update_binding: None,
                update_account_snapshot: None,
                deadline: OAuthSetupDeadline::GateDefault,
            })
            .await?;
        let prepared = self
            .restore_setup_original(request.flow_manager, original, None)
            .await?;
        challenge_view_from_flow(&prepared.flow)
    }

    async fn pkce_verifier_for_flow(
        &self,
        flow_manager: &Arc<dyn AuthFlowManager>,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<SecretString>, AuthProductError> {
        let Some(state) = flow_manager.read_oauth_setup(scope, flow_id).await? else {
            // Legacy verifiers without captured client bytes cannot establish
            // the first client configuration. Explicit reauthorization is needed.
            return Ok(None);
        };
        if state.flow.expires_at <= Utc::now()
            || brassclaw_auth::is_terminal_status(state.flow.status)
        {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        let (original, payload) = self
            .original_private_material(&scope.resource, flow_id)
            .await?;
        state.original.verify(&original)?;
        Ok(Some(payload.pkce))
    }
}

impl fmt::Debug for GoogleOAuthGateProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GoogleOAuthGateProvider")
            .field("client_id", &self.client.client_id.as_str())
            .field("redirect_uri", &self.client.redirect_uri)
            .finish()
    }
}

fn stable_gate_identity(
    scope: &AuthProductScope,
    run: TurnRunId,
    gate: &AuthGateRef,
    predecessors: &[AuthFlowId],
) -> Result<uuid::Uuid, AuthProductError> {
    let key = serde_json::json!({
        "format":"google-gate-setup-identity/1",
        "tenant":scope.resource.tenant_id,
        "user":scope.resource.user_id,
        "agent":scope.resource.agent_id,
        "project":scope.resource.project_id,
        "thread":scope.resource.thread_id,
        "run":run.to_string(),
        "gate":gate,
        "predecessors":predecessors,
    });
    let bytes = serde_json::to_vec(&key).map_err(|_| AuthProductError::BackendUnavailable)?;
    let hash = brassclaw_common::hashing::sha256_hex(&bytes);
    uuid::Uuid::parse_str(&hash[..32]).map_err(|_| AuthProductError::BackendUnavailable)
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
        AuthSurface::Callback,
    )
}

fn turn_gate_query(
    auth_scope: &AuthProductScope,
    turn_scope: &TurnScope,
    turn_run_ref: &TurnRunRef,
    gate_ref: &AuthGateRef,
) -> TurnGateAuthFlowQuery {
    TurnGateAuthFlowQuery {
        owner: AuthFlowOwnerScope {
            tenant_id: auth_scope.resource.tenant_id.clone(),
            user_id: auth_scope.resource.user_id.clone(),
            agent_id: auth_scope.resource.agent_id.clone(),
            project_id: auth_scope.resource.project_id.clone(),
            thread_id: turn_scope.thread_id.clone(),
        },
        turn_run_ref: turn_run_ref.clone(),
        gate_ref: gate_ref.clone(),
        include_terminal: false,
    }
}

fn provider_scopes(raw_scopes: &[String]) -> Result<Vec<ProviderScope>, AuthProductError> {
    raw_scopes
        .iter()
        .map(|scope| ProviderScope::new(scope.clone()))
        .collect()
}

fn challenge_view_from_flow(flow: &AuthFlowRecord) -> Result<AuthChallengeView, AuthProductError> {
    match flow.challenge.as_ref() {
        Some(AuthChallenge::OAuthUrl {
            authorization_url,
            expires_at,
        }) => Ok(AuthChallengeView {
            kind: AuthPromptChallengeKind::OAuthUrl,
            provider: flow.provider.clone(),
            account_label: None,
            authorization_url: Some(authorization_url.clone()),
            expires_at: Some(*expires_at),
        }),
        Some(_) | None => Err(AuthProductError::BackendUnavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_auth::{
        AuthFlowKind, AuthFlowStatus, GOOGLE_CALENDAR_READONLY_SCOPE, InMemoryAuthProductServices,
        NewAuthFlow, OAuthAuthorizationUrl,
    };
    use brassclaw_host_api::{
        AgentId, ExtensionId, RuntimeCredentialAccountProviderId, TenantId, ThreadId, UserId,
    };
    use brassclaw_secrets::InMemorySecretStore;

    #[tokio::test]
    async fn google_oauth_gate_replaces_expired_turn_gate_flow() {
        let fixture = GateFixture::new(None);
        let expired_flow_id = AuthFlowId::new();
        let expired_scope = fixture.auth_scope();
        fixture
            .flow_manager
            .create_flow(NewAuthFlow {
                requirement_setup: None,
                id: Some(expired_flow_id),
                scope: expired_scope.clone(),
                kind: AuthFlowKind::IntegrationCredential,
                provider: AuthProviderId::new(brassclaw_auth::GOOGLE_PROVIDER_ID).unwrap(),
                challenge: AuthChallenge::OAuthUrl {
                    authorization_url: OAuthAuthorizationUrl::new(
                        "https://accounts.google.com/o/oauth2/v2/auth?state=expired".to_string(),
                    )
                    .unwrap(),
                    expires_at: Utc::now() - ChronoDuration::seconds(1),
                },
                continuation: AuthContinuationRef::TurnGateResume {
                    turn_run_ref: TurnRunRef::new(fixture.run_id.to_string()).unwrap(),
                    gate_ref: fixture.gate_ref.clone(),
                },
                update_binding: None,
                opaque_state_hash: None,
                pkce_verifier_hash: None,
                expires_at: Utc::now() - ChronoDuration::seconds(1),
            })
            .await
            .unwrap();

        let challenge = fixture.challenge().await;

        assert_ne!(
            challenge.authorization_url.unwrap().as_str(),
            "https://accounts.google.com/o/oauth2/v2/auth?state=expired"
        );
        let expired = fixture
            .shared
            .get_flow(&expired_scope, expired_flow_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(expired.status, AuthFlowStatus::Canceled);
        assert_eq!(fixture.active_gate_flows().await.len(), 1);
    }

    #[tokio::test]
    async fn google_oauth_gate_reuses_one_flow_under_concurrent_challenges() {
        let fixture = GateFixture::new(None);

        let (left, right) = tokio::join!(fixture.challenge(), fixture.challenge());
        let left = left.authorization_url.unwrap();
        let right = right.authorization_url.unwrap();

        assert_eq!(left, right);
        assert_eq!(fixture.active_gate_flows().await.len(), 1);
    }

    #[tokio::test]
    async fn google_gate_independent_owners_converge_without_process_local_locks() {
        let fixture = GateFixture::new(None);
        let independent = GoogleOAuthGateProvider::new(
            OAuthClientConfig::new(
                "other-google-client",
                "https://other.example/google/callback",
                None,
            )
            .unwrap(),
            fixture.provider.secret_store.clone(),
        );
        let requirements = vec![fixture.requirement.clone()];
        let (first, second) = tokio::join!(
            fixture.challenge(),
            independent.challenge_for_blocked_gate(
                OAuthGateChallengeRequest {
                    flow_manager: &fixture.flow_manager,
                    flow_source: &fixture.flow_source,
                    requirements: &requirements,
                    scope: &fixture.scope,
                    owner_user_id: &fixture.owner_user_id,
                    run_id: fixture.run_id,
                    gate_ref: &fixture.gate_ref,
                },
                &fixture.requirement,
            )
        );
        let second = second.unwrap();
        assert_eq!(first.authorization_url, second.authorization_url);
        assert_eq!(first.expires_at, second.expires_at);
        assert_eq!(fixture.active_gate_flows().await.len(), 1);
    }

    #[tokio::test]
    async fn google_gate_new_owner_keeps_first_client_challenge_and_expiry() {
        let fixture = GateFixture::new(None);
        let first = fixture.challenge().await;
        let changed = GoogleOAuthGateProvider::new(
            OAuthClientConfig::new("later-client", "https://later.example/callback", None).unwrap(),
            fixture.provider.secret_store.clone(),
        );
        let requirements = vec![fixture.requirement.clone()];
        let second = changed
            .challenge_for_blocked_gate(
                OAuthGateChallengeRequest {
                    flow_manager: &fixture.flow_manager,
                    flow_source: &fixture.flow_source,
                    requirements: &requirements,
                    scope: &fixture.scope,
                    owner_user_id: &fixture.owner_user_id,
                    run_id: fixture.run_id,
                    gate_ref: &fixture.gate_ref,
                },
                &fixture.requirement,
            )
            .await
            .unwrap();
        assert_eq!(first.authorization_url, second.authorization_url);
        assert_eq!(first.expires_at, second.expires_at);
        assert_eq!(fixture.active_gate_flows().await.len(), 1);
    }

    #[tokio::test]
    async fn google_oauth_gate_authorization_url_keeps_hosted_domain_hint() {
        let fixture = GateFixture::new(Some("example.com"));

        let challenge = fixture.challenge().await;
        let authorization_url = challenge.authorization_url.unwrap();
        let parsed = url::Url::parse(authorization_url.as_str()).unwrap();

        assert!(
            parsed
                .query_pairs()
                .any(|(name, value)| name == "hd" && value == "example.com")
        );
    }

    #[tokio::test]
    async fn google_oauth_gate_registry_uses_registered_requirement_when_multiple_present() {
        let fixture = GateFixture::new(None);
        let registry =
            GoogleOAuthGateProviderRegistry::new(vec![Arc::new(fixture.provider.clone())]);
        let unsupported_requirement = RuntimeCredentialAuthRequirement {
            requirement: None,
            provider: RuntimeCredentialAccountProviderId::new("github").unwrap(),
            requester_extension: ExtensionId::new("github").unwrap(),
            provider_scopes: Vec::new(),
        };
        let requirements = vec![unsupported_requirement, fixture.requirement.clone()];

        let challenge = registry
            .challenge_for_blocked_gate(OAuthGateChallengeRequest {
                flow_manager: &fixture.flow_manager,
                flow_source: &fixture.flow_source,
                requirements: &requirements,
                scope: &fixture.scope,
                owner_user_id: &fixture.owner_user_id,
                run_id: fixture.run_id,
                gate_ref: &fixture.gate_ref,
            })
            .await
            .unwrap()
            .expect("google requirement should produce a challenge");

        assert_eq!(challenge.kind, AuthPromptChallengeKind::OAuthUrl);
        assert_eq!(fixture.active_gate_flows().await.len(), 1);
    }

    struct GateFixture {
        shared: Arc<InMemoryAuthProductServices>,
        flow_manager: Arc<dyn AuthFlowManager>,
        flow_source: Arc<dyn AuthFlowRecordSource>,
        provider: GoogleOAuthGateProvider,
        scope: TurnScope,
        owner_user_id: UserId,
        run_id: TurnRunId,
        gate_ref: AuthGateRef,
        requirement: RuntimeCredentialAuthRequirement,
    }

    impl GateFixture {
        fn new(hosted_domain_hint: Option<&str>) -> Self {
            let shared = Arc::new(InMemoryAuthProductServices::new());
            let flow_manager: Arc<dyn AuthFlowManager> = shared.clone();
            let flow_source: Arc<dyn AuthFlowRecordSource> = shared.clone();
            let mut client = OAuthClientConfig::new(
                "google-client.apps.googleusercontent.com",
                "http://127.0.0.1:3000/api/reborn/product-auth/oauth/google/callback",
                None,
            )
            .unwrap();
            if let Some(hosted_domain_hint) = hosted_domain_hint {
                client = client.with_hosted_domain_hint(hosted_domain_hint);
            }
            Self {
                shared,
                flow_manager,
                flow_source,
                provider: GoogleOAuthGateProvider::new(
                    client,
                    Arc::new(InMemorySecretStore::new()),
                ),
                scope: TurnScope::new(
                    TenantId::new("tenant-alpha").unwrap(),
                    Some(AgentId::new("agent-alpha").unwrap()),
                    None,
                    ThreadId::new("thread-alpha").unwrap(),
                ),
                owner_user_id: UserId::new("user-alpha").unwrap(),
                run_id: TurnRunId::new(),
                gate_ref: AuthGateRef::new("gate:google-auth").unwrap(),
                requirement: RuntimeCredentialAuthRequirement {
                    requirement: None,
                    provider: RuntimeCredentialAccountProviderId::new("google").unwrap(),
                    requester_extension: ExtensionId::new("google-calendar").unwrap(),
                    provider_scopes: vec![GOOGLE_CALENDAR_READONLY_SCOPE.to_string()],
                },
            }
        }

        async fn challenge(&self) -> AuthChallengeView {
            self.provider
                .challenge_for_blocked_gate(
                    OAuthGateChallengeRequest {
                        flow_manager: &self.flow_manager,
                        flow_source: &self.flow_source,
                        requirements: std::slice::from_ref(&self.requirement),
                        scope: &self.scope,
                        owner_user_id: &self.owner_user_id,
                        run_id: self.run_id,
                        gate_ref: &self.gate_ref,
                    },
                    &self.requirement,
                )
                .await
                .unwrap()
        }

        fn auth_scope(&self) -> AuthProductScope {
            auth_scope_for_blocked_turn(&self.scope, &self.owner_user_id)
        }

        async fn active_gate_flows(&self) -> Vec<AuthFlowRecord> {
            let auth_scope = self.auth_scope();
            let turn_run_ref = TurnRunRef::new(self.run_id.to_string()).unwrap();
            let query = turn_gate_query(&auth_scope, &self.scope, &turn_run_ref, &self.gate_ref);
            self.shared
                .flows_for_owner(query.owner)
                .await
                .unwrap()
                .into_iter()
                .filter(|flow| {
                    flow.status == AuthFlowStatus::AwaitingUser
                        && matches!(
                            flow.continuation,
                            AuthContinuationRef::TurnGateResume { .. }
                        )
                })
                .collect()
        }
    }
}
