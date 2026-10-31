//! Full protected setup/public callback handoff through actual Auth/Secret owners.
use super::*;
use brassclaw_secrets::SecretStore;

async fn callback(app: &Router, original_url: &str) -> (StatusCode, serde_json::Value) {
    let authorization = Url::parse(original_url).unwrap();
    let value = |key: &str| {
        authorization
            .query_pairs()
            .find_map(|(name, value)| (name == key).then(|| value.into_owned()))
            .unwrap()
    };
    let mut target = Url::parse(&value("redirect_uri")).unwrap();
    target
        .query_pairs_mut()
        .append_pair("state", &value("state"))
        .append_pair("code", "controlled-usage-code");
    let uri = format!("{}?{}", target.path(), target.query().unwrap());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(uri)
                .header(header::ACCEPT, "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

pub(crate) async fn verify_dcr_oauth_usage(
    ports: crate::auth::RebornProductAuthServicePorts,
    secrets: Arc<dyn SecretStore>,
) {
    let observed = Arc::new(ObservedRegistration {
        registrations: AtomicUsize::new(0),
        token_exchanges: AtomicUsize::new(0),
        deletions: AtomicUsize::new(0),
        malformed: false,
        cleanup_status: 204,
        cleanup_unknown: false,
    });
    let provider = Arc::new(
        OAuthDcrProvider::new(
            OAuthDcrProviderConfig {
                spec: crate::notion_oauth::notion_provider_spec(),
                callback_origin: "http://127.0.0.1:3000".into(),
                client_name: "Brassclaw".into(),
                account_label: CredentialAccountLabel::new("notion").unwrap(),
                scopes: vec![],
            },
            observed.clone(),
            secrets.clone(),
            Arc::new(NoopObligationHandler),
        )
        .unwrap(),
    );
    let client = crate::oauth_provider_client::HostOAuthProviderClient::new_with_client_material(
        crate::notion_oauth::notion_provider_spec(),
        observed.clone(),
        secrets,
        Arc::new(NoopObligationHandler),
        provider.clone(),
    )
    .unwrap();
    let auth = Arc::new(
        ports
            .with_provider_client(Arc::new(client))
            .into_services(Arc::new(NoopDispatcher))
            .with_dcr_oauth_registry(Arc::new(OAuthDcrProviderRegistry::new(vec![provider]))),
    );
    let state = ProductAuthRouteState::new(
        auth.clone(),
        TenantId::new("tenant-alpha").unwrap(),
        None,
        None,
    )
    .with_services(extension_oauth_requirement::test_services("notion"));
    let mount = product_auth_route_mount(state.clone());
    let app = mount
        .protected
        .merge(mount.public)
        .layer(axum::Extension(test_caller()));
    let original = request().await;
    let (status, first) = send(&app, original.clone()).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let id = AuthFlowId::from_uuid(Uuid::parse_str(first["flow_id"].as_str().unwrap()).unwrap());
    let scope = scope_from_authenticated_caller_parts_requiring_invocation(
        &test_caller(),
        &ScopeFields {
            session_id: None,
            thread_id: None,
            invocation_id: Some(original["invocation_id"].as_str().unwrap().into()),
        },
    )
    .unwrap();
    let requirement = extension_oauth_requirement::test_requirement("notion");
    let setup = auth
        .flow_manager()
        .requirement_setup(&scope, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(setup.requirement(), &requirement);
    assert!(setup.previous().is_none());
    let registration = auth
        .flow_manager()
        .read_dcr_registration(&scope, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        registration.action.original.requirement_setup.as_deref(),
        Some(&setup)
    );
    assert_eq!(
        registration.action.original.preparation_context()["requirement_setup"],
        serde_json::to_value(&setup).unwrap()
    );
    let first_url = first["authorization_url"].as_str().unwrap();
    let (status, result) = callback(&app, first_url).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let first_flow = auth
        .flow_manager()
        .get_flow(&scope, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first_flow.status, AuthFlowStatus::Completed);
    let selected = auth
        .credential_account_record_source()
        .requirement_selection(&scope, &requirement)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.requirement(), &requirement);
    assert_eq!(
        first_flow.credential_account_id,
        Some(selected.account().id)
    );
    assert_eq!(observed.token_exchanges.load(Ordering::SeqCst), 1);
    let (_, repeated) = callback(&app, first_url).await;
    assert_eq!(repeated, result);
    assert_eq!(observed.token_exchanges.load(Ordering::SeqCst), 1);

    let mut successor = request().await;
    successor["target"] = json!({"kind":"existing","account_id":selected.account().id});
    let (status, second) = send(&app, successor.clone()).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    let next_id =
        AuthFlowId::from_uuid(Uuid::parse_str(second["flow_id"].as_str().unwrap()).unwrap());
    let retained = auth
        .flow_manager()
        .requirement_setup(&scope, next_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.requirement(), &requirement);
    assert_eq!(retained.previous(), Some(&selected));
    let flow = auth
        .flow_manager()
        .get_flow(&scope, next_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        flow.update_binding.as_ref().unwrap().account_id,
        selected.account().id
    );
    let mut changed = successor.clone();
    changed["target"] = json!({"kind":"new"});
    assert_eq!(send(&app, changed).await.0, StatusCode::CONFLICT);
    let mut missing_source = state;
    missing_source.services = None;
    let recovered = product_auth_route_mount(missing_source)
        .protected
        .layer(axum::Extension(test_caller()));
    let mut recover = successor.clone();
    recover["action"] = json!("recover");
    assert_eq!(send(&recovered, recover).await.1, second);
    assert_eq!(observed.registrations.load(Ordering::SeqCst), 2);
    let (status, result) = callback(&app, second["authorization_url"].as_str().unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let latest = auth
        .credential_account_record_source()
        .requirement_selection(&scope, &requirement)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest.account().id, selected.account().id);
    assert_eq!(latest.previous(), Some(&selected.reference().unwrap()));
    assert_ne!(latest.reference().unwrap(), selected.reference().unwrap());
    assert_eq!(observed.token_exchanges.load(Ordering::SeqCst), 2);
    callback(&app, first_url).await;
    assert_eq!(observed.token_exchanges.load(Ordering::SeqCst), 2);
    assert_eq!(
        auth.credential_account_record_source()
            .requirement_selection(&scope, &requirement)
            .await
            .unwrap()
            .unwrap(),
        latest
    );
}

#[path = "oauth_preparation_tests.rs"]
mod preparation;
pub(crate) use preparation::{DcrPreparationBackend, verify_dcr_preparation_handoff};
