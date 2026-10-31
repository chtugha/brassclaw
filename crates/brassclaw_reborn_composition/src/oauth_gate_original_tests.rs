//! Same Google setup/provider/facade path on actual native encrypted owners.
//! Controlled HTTP replies establish transport inputs; no live issuer is claimed.
use super::*;
use crate::auth::{RebornAuthContinuationDispatcher, RebornProductAuthServicePorts};
use crate::oauth_gate::GoogleOAuthGateProviderRegistry;
use crate::oauth_provider_client::{HostOAuthProviderClient, OAuthClientMaterialSource};
use brassclaw_auth::{
    AuthContinuationEvent, AuthSurface, CredentialAccountLookupRequest, CredentialRefreshRequest,
};
use brassclaw_capabilities::{
    CapabilityObligationError, CapabilityObligationHandler, CapabilityObligationRequest,
};
use brassclaw_host_api::{
    RuntimeHttpEgress, RuntimeHttpEgressError, RuntimeHttpEgressRequest, RuntimeHttpEgressResponse,
};
use std::sync::Mutex;

struct NoopDispatcher;
#[async_trait::async_trait]
impl RebornAuthContinuationDispatcher for NoopDispatcher {
    async fn dispatch_auth_continuation(
        &self,
        _: AuthContinuationEvent,
    ) -> Result<(), AuthProductError> {
        Ok(())
    }
}
struct NoopObligations;
#[async_trait::async_trait]
impl CapabilityObligationHandler for NoopObligations {
    async fn satisfy(
        &self,
        _: CapabilityObligationRequest<'_>,
    ) -> Result<(), CapabilityObligationError> {
        Ok(())
    }
}
#[derive(Default)]
struct ObservedHttp {
    requests: Mutex<Vec<RuntimeHttpEgressRequest>>,
}
#[async_trait::async_trait]
impl RuntimeHttpEgress for ObservedHttp {
    async fn execute(
        &self,
        request: RuntimeHttpEgressRequest,
    ) -> Result<RuntimeHttpEgressResponse, RuntimeHttpEgressError> {
        let mut requests = self.requests.lock().unwrap();
        assert_eq!(request.url, brassclaw_auth::GOOGLE_TOKEN_ENDPOINT);
        requests.push(request);
        let body = if requests.len() == 1 {
            br#"{"access_token":"controlled-first-access","refresh_token":"controlled-first-refresh","expires_in":3600,"scope":"https://www.googleapis.com/auth/calendar.readonly"}"#.to_vec()
        } else {
            br#"{"access_token":"controlled-rotated-access","refresh_token":"controlled-rotated-refresh","expires_in":3600,"scope":"https://www.googleapis.com/auth/calendar.readonly"}"#.to_vec()
        };
        Ok(RuntimeHttpEgressResponse {
            status: 200,
            headers: vec![("content-type".into(), "application/json".into())],
            body,
            saved_body: None,
            request_bytes: 1,
            response_bytes: 1,
            redaction_applied: true,
        })
    }
}

async fn public_callback(
    app: &axum::Router,
    authorization: &brassclaw_auth::OAuthAuthorizationUrl,
) -> crate::auth::RebornOAuthCallbackResponse {
    use tower::ServiceExt;
    let authorization = url::Url::parse(authorization.as_str()).unwrap();
    let state = authorization
        .query_pairs()
        .find_map(|(key, value)| (key == "state").then(|| value.into_owned()))
        .unwrap();
    let redirect = authorization
        .query_pairs()
        .find_map(|(key, value)| (key == "redirect_uri").then(|| value.into_owned()))
        .unwrap();
    let mut target = url::Url::parse(&redirect).unwrap();
    target
        .query_pairs_mut()
        .append_pair("state", &state)
        .append_pair("code", "controlled-google-code");
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("{}?{}", target.path(), target.query().unwrap()))
                .header(axum::http::header::ACCEPT, "application/json")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&body)
    );
    serde_json::from_slice(&body).unwrap()
}

fn provider(
    client: &str,
    secret: &str,
    secrets: Arc<dyn SecretStore>,
) -> Arc<GoogleOAuthGateProvider> {
    Arc::new(GoogleOAuthGateProvider::new(
        OAuthClientConfig::new(
            client,
            "https://app.example/api/reborn/product-auth/oauth/google/callback",
            Some(secret.into()),
        )
        .unwrap(),
        secrets,
    ))
}

pub(crate) async fn verify_native_private_runtime(
    first: RebornProductAuthServicePorts,
    second: RebornProductAuthServicePorts,
    first_secrets: Arc<dyn SecretStore>,
    second_secrets: Arc<dyn SecretStore>,
    pool: &brassclaw_pg::PgPool,
    backend: crate::auth_oauth_setup_custody::tests::Backend,
    scope: AuthProductScope,
) {
    let scope = AuthProductScope::new(scope.resource, AuthSurface::Callback);
    let first_provider = provider(
        "first-client",
        "controlled-first-client-secret",
        first_secrets.clone(),
    );
    let first = first
        .into_services(Arc::new(NoopDispatcher))
        .with_oauth_gate_registry(Arc::new(GoogleOAuthGateProviderRegistry::new(vec![
            first_provider.clone(),
        ])));
    let id = AuthFlowId::new();
    let expiry = Utc::now() + ChronoDuration::minutes(5);
    let scopes = vec![ProviderScope::new(brassclaw_auth::GOOGLE_CALENDAR_READONLY_SCOPE).unwrap()];
    let label = CredentialAccountLabel::new("native original Google client").unwrap();
    let owner = brassclaw_host_api::ExtensionId::new("native-google-owner").unwrap();
    let prior_access = SecretHandle::new(format!("native-owned-google-before-{id}")).unwrap();
    first_secrets
        .put(
            scope.resource.clone(),
            prior_access.clone(),
            "controlled-before-reauthorization".into(),
        )
        .await
        .unwrap();
    let bound = first
        .credential_account_service()
        .create_account(brassclaw_auth::NewCredentialAccount {
            scope: scope.clone(),
            provider: AuthProviderId::new("google").unwrap(),
            label: label.clone(),
            status: brassclaw_auth::CredentialAccountStatus::Expired,
            ownership: brassclaw_auth::CredentialOwnership::ExtensionOwned,
            owner_extension: Some(owner.clone()),
            granted_extensions: vec![],
            access_secret: Some(prior_access),
            access_expires_at: None,
            refresh_secret: None,
            scopes: scopes.clone(),
        })
        .await
        .unwrap();
    let requirement = brassclaw_host_api::RuntimeCredentialAccountRequirement::new(
        owner.clone(),
        brassclaw_host_api::RuntimeCredentialRequirement {
            handle: SecretHandle::new("native-google-calendar").unwrap(),
            source: brassclaw_host_api::RuntimeCredentialRequirementSource::ProductAuthAccount {
                provider: brassclaw_host_api::RuntimeCredentialAccountProviderId::new("google")
                    .unwrap(),
                setup: brassclaw_host_api::RuntimeCredentialAccountSetup::OAuth {
                    scopes: scopes
                        .iter()
                        .map(|scope| scope.as_str().to_owned())
                        .collect(),
                },
            },
            provider_scopes: scopes
                .iter()
                .map(|scope| scope.as_str().to_owned())
                .collect(),
            audience: brassclaw_host_api::NetworkTargetPattern {
                scheme: Some(brassclaw_host_api::NetworkScheme::Https),
                host_pattern: "www.googleapis.com".into(),
                port: None,
            },
            target: brassclaw_host_api::RuntimeCredentialTarget::Header {
                name: "Authorization".into(),
                prefix: Some("Bearer ".into()),
            },
            required: true,
        },
    )
    .unwrap();
    let (setup, binding) = first
        .prepare_extension_oauth_requirement(&scope, requirement, Some(bound.id))
        .await
        .unwrap();
    let mut request = GoogleSetupRequest {
        scope: scope.clone(),
        flow_id: id,
        account_label: label.clone(),
        requested_scopes: scopes.clone(),
        continuation: AuthContinuationRef::SetupOnly,
        requirement_setup: Some(Box::new(setup)),
        update_binding: binding,
        update_account_snapshot: None,
        deadline: OAuthSetupDeadline::Exact(expiry),
    };
    let rejection = match backend {
        crate::auth_oauth_setup_custody::tests::Backend::NativePostgres => {
            "CREATE FUNCTION reject_google_setup_insert() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'controlled first Google flow rejection'; END $$; CREATE TRIGGER reject_google_setup_insert BEFORE INSERT ON brassclaw_product_auth_flows FOR EACH ROW EXECUTE FUNCTION reject_google_setup_insert();"
        }
        #[cfg(feature = "skills-db")]
        crate::auth_oauth_setup_custody::tests::Backend::RecordFilesystem => {
            "CREATE FUNCTION reject_google_setup_insert() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.path LIKE '%/flows/%' THEN RAISE EXCEPTION 'controlled first Google flow rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_google_setup_insert BEFORE INSERT OR UPDATE ON root_filesystem_entries FOR EACH ROW EXECUTE FUNCTION reject_google_setup_insert();"
        }
    };
    let sql = pool.get().await.unwrap();
    sql.batch_execute(rejection).await.unwrap();
    assert!(
        first
            .start_captured_google_setup(
                request.clone(),
                &brassclaw_auth::GoogleOAuthRouteConfig::new(
                    "first-client",
                    "https://app.example/api/reborn/product-auth/oauth/google/callback"
                )
                .unwrap(),
            )
            .await
            .is_err(),
        "the actual first Auth-flow write must fail after encrypted preparation"
    );
    assert!(
        first
            .flow_manager()
            .get_flow(&scope, id)
            .await
            .unwrap()
            .is_none()
    );
    let first_original = first_provider
        .retained_setup_original(&scope, id)
        .await
        .unwrap()
        .unwrap();
    let drop_trigger = match backend {
        crate::auth_oauth_setup_custody::tests::Backend::NativePostgres => {
            "DROP TRIGGER reject_google_setup_insert ON brassclaw_product_auth_flows; DROP FUNCTION reject_google_setup_insert();"
        }
        #[cfg(feature = "skills-db")]
        crate::auth_oauth_setup_custody::tests::Backend::RecordFilesystem => {
            "DROP TRIGGER reject_google_setup_insert ON root_filesystem_entries; DROP FUNCTION reject_google_setup_insert();"
        }
    };
    sql.batch_execute(drop_trigger).await.unwrap();
    drop(sql);
    request.update_account_snapshot = first_original.definition.update_account_snapshot.clone();
    // Cancel a separate prepared setup before any output alias exists. The
    // existing identity-closure primitive must prevent a late first use.
    let cancel_id = AuthFlowId::new();
    let mut cancel_request = request.clone();
    cancel_request.flow_id = cancel_id;
    let cancel_flow = first
        .start_captured_google_setup(
            cancel_request,
            &brassclaw_auth::GoogleOAuthRouteConfig::new(
                "first-client",
                "https://app.example/api/reborn/product-auth/oauth/google/callback",
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let canceled_original = first_provider
        .retained_setup_original(&scope, cancel_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        first
            .cancel_google_setup_original(&scope, cancel_id, &label, &scopes, expiry)
            .await
            .unwrap()
            .status,
        brassclaw_auth::AuthFlowStatus::Canceled
    );
    assert!(matches!(
        first_secrets
            .custody(
                &scope.resource,
                &canceled_original.definition.exchange_use_handle().unwrap()
            )
            .await
            .unwrap()
            .unwrap()
            .state,
        SecretCustodyState::ClosedIdentity { .. }
    ));
    assert!(
        first_provider
            .exchange_material(&scope.resource, cancel_flow.id)
            .await
            .is_err()
    );

    let initial_snapshot = serde_json::to_string(&first_original).unwrap();
    assert!(!initial_snapshot.contains("controlled-first-client-secret"));
    assert!(!initial_snapshot.contains("client_secret"));

    // Recreate both host client and service facade using a different configured
    // client. The actual callback must still send the original client's bytes.
    let observed = Arc::new(ObservedHttp::default());
    let replacement = provider(
        "later-client",
        "controlled-later-client-secret",
        second_secrets.clone(),
    );
    let client = HostOAuthProviderClient::new_with_client_material(
        crate::google_oauth::google_provider_spec(),
        observed.clone(),
        second_secrets.clone(),
        Arc::new(NoopObligations),
        replacement.clone(),
    )
    .unwrap();
    let second = Arc::new(
        second
            .with_provider_client(Arc::new(client))
            .into_services(Arc::new(NoopDispatcher))
            .with_oauth_gate_registry(Arc::new(GoogleOAuthGateProviderRegistry::new(vec![
                replacement.clone(),
            ]))),
    );
    let first_flow = second
        .recover_google_setup_original(&scope, id, &label, &scopes, expiry)
        .await
        .unwrap()
        .unwrap();
    first_original.definition.verify_flow(&first_flow).unwrap();
    assert_eq!(
        second
            .recover_google_setup_original(&scope, id, &label, &scopes, expiry)
            .await
            .unwrap(),
        Some(first_flow)
    );
    assert_eq!(
        replacement.prepare_setup_original(request).await.unwrap(),
        first_original
    );
    let absent_refresh = SecretHandle::new(format!("unused-google-refresh-{id}")).unwrap();
    replacement
        .cleanup_refresh_material(&scope.resource, &absent_refresh)
        .await
        .unwrap();
    assert!(
        replacement
            .bind_refresh_material(&scope.resource, id, &absent_refresh)
            .await
            .is_err(),
        "late client binding cannot reuse a closed absent identity"
    );
    let app = crate::product_auth_serve::product_auth_route_mount(
        crate::product_auth_serve::ProductAuthRouteState::new(
            second.clone(),
            scope.resource.tenant_id.clone(),
            None,
            None,
        ),
    )
    .public;
    let completed = public_callback(&app, &first_original.definition.authorization_url).await;
    assert_eq!(completed.status, brassclaw_auth::AuthFlowStatus::Completed);
    assert_eq!(
        public_callback(&app, &first_original.definition.authorization_url).await,
        completed
    );
    assert_eq!(
        observed.requests.lock().unwrap().len(),
        1,
        "recovery does not resubmit the issuer"
    );
    let account_id = completed.credential_account_id.unwrap();
    assert_eq!(account_id, bound.id);
    let account = second
        .credential_account_service()
        .get_account(
            CredentialAccountLookupRequest::new(scope.clone(), account_id)
                .for_extension(owner.clone()),
        )
        .await
        .unwrap()
        .unwrap();
    let old_refresh = account.refresh_secret.unwrap();
    let raw_handle = first_original.definition.exchange_use_handle().unwrap();
    assert!(matches!(
        second_secrets
            .custody(&scope.resource, &raw_handle)
            .await
            .unwrap()
            .unwrap()
            .state,
        SecretCustodyState::RetiredRetained { .. }
    ));
    assert!(matches!(
        second_secrets
            .custody(&scope.resource, &first_original.preparation.handle)
            .await
            .unwrap()
            .unwrap()
            .state,
        SecretCustodyState::RetiredOriginal { .. }
    ));
    assert!(
        second_secrets
            .lease_once(&scope.resource, &raw_handle)
            .await
            .is_err()
    );
    let material = replacement
        .refresh_material(&scope.resource, &old_refresh)
        .await
        .unwrap();
    assert_eq!(material.client_id.as_str(), "first-client");
    assert_eq!(
        material.client_secret.unwrap().expose_secret(),
        "controlled-first-client-secret"
    );
    assert!(
        replacement
            .exchange_material(&scope.resource, id)
            .await
            .is_err()
    );

    let refresh = second
        .refresh_credential_account(
            CredentialRefreshRequest::new(
                scope.clone(),
                AuthProviderId::new("google").unwrap(),
                account_id,
            )
            .for_extension(owner.clone()),
        )
        .await
        .unwrap();
    assert!(refresh.refreshed);
    let account = second
        .credential_account_service()
        .get_account(
            CredentialAccountLookupRequest::new(scope.clone(), account_id)
                .for_extension(owner.clone()),
        )
        .await
        .unwrap()
        .unwrap();
    let rotated = account.refresh_secret.unwrap();
    assert_ne!(rotated, old_refresh);
    replacement
        .cleanup_refresh_material(&scope.resource, &old_refresh)
        .await
        .unwrap();
    replacement
        .cleanup_refresh_material(&scope.resource, &old_refresh)
        .await
        .unwrap();
    let selected = replacement
        .refresh_material(&scope.resource, &rotated)
        .await
        .unwrap();
    assert_eq!(selected.client_id.as_str(), "first-client");
    assert!(
        replacement
            .refresh_material(&scope.resource, &old_refresh)
            .await
            .is_err()
    );
    let cleanup = second
        .cleanup_credentials_for_lifecycle(brassclaw_auth::SecretCleanupRequest {
            scope: scope.clone(),
            extension_id: owner.clone(),
            action: brassclaw_auth::SecretCleanupAction::Uninstall,
        })
        .await
        .unwrap();
    assert_eq!(cleanup.revoked_accounts, vec![account_id]);
    assert!(cleanup.quarantined_accounts.is_empty());
    assert!(
        second_secrets
            .lease_once(&scope.resource, &rotated)
            .await
            .is_err()
    );
    assert!(
        replacement
            .refresh_material(&scope.resource, &rotated)
            .await
            .is_err(),
        "ordinary account cleanup also retires its exact private client bindings"
    );
    let again = second
        .cleanup_credentials_for_lifecycle(brassclaw_auth::SecretCleanupRequest {
            scope: scope.clone(),
            extension_id: owner,
            action: brassclaw_auth::SecretCleanupAction::Uninstall,
        })
        .await
        .unwrap();
    assert!(again.quarantined_accounts.is_empty());
    let requests = observed.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    for request in requests.iter() {
        let fields = url::form_urlencoded::parse(&request.body)
            .into_owned()
            .collect::<BTreeMap<_, _>>();
        assert_eq!(fields["client_id"], "first-client");
        assert_eq!(fields["client_secret"], "controlled-first-client-secret");
        assert!(
            !request
                .body
                .windows(b"later-client".len())
                .any(|part| part == b"later-client")
        );
    }
}
