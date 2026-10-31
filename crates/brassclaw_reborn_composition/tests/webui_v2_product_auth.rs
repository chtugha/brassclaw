//! Public facade ownership contract. The complete caller cases live in the
//! owning crate's default tests so their host fixtures need no public secret seam.
use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use brassclaw_auth::{AuthContinuationEvent, AuthProductError, InMemoryAuthProductServices};
use brassclaw_host_api::{InvocationId, TenantId, UserId};
use brassclaw_reborn_composition::{
    GoogleOAuthRouteConfig, RebornAuthContinuationDispatcher, RebornProductAuthServices,
    RebornReadiness, RebornWebuiBundle, WebuiAuthenticator, WebuiServeConfig, webui_v2_app,
};
use tower::ServiceExt;

struct Token;
#[async_trait]
impl WebuiAuthenticator for Token {
    async fn authenticate(&self, token: &str) -> Option<UserId> {
        (token == "operator-token").then(|| UserId::new("public-auth-owner").unwrap())
    }
}
struct Noop;
#[async_trait]
impl RebornAuthContinuationDispatcher for Noop {
    async fn dispatch_auth_continuation(
        &self,
        _: AuthContinuationEvent,
    ) -> Result<(), AuthProductError> {
        Ok(())
    }
}

#[tokio::test]
async fn public_google_route_metadata_cannot_substitute_for_a_host_original_owner() {
    let api = brassclaw_product_workflow::RebornServices::new(
        Arc::new(brassclaw_threads::InMemorySessionThreadService::default()),
        Arc::new(brassclaw_turns::DefaultTurnCoordinator::new(Arc::new(
            brassclaw_turns::InMemoryTurnStateStore::default(),
        ))),
    );
    let bundle = RebornWebuiBundle {
        api: Arc::new(api),
        product_auth: Some(Arc::new(RebornProductAuthServices::from_shared(
            Arc::new(InMemoryAuthProductServices::new()),
            Arc::new(Noop),
        ))),
        readiness: RebornReadiness::disabled(),
    };
    let config = WebuiServeConfig::new(
        TenantId::new("public-auth-tenant").unwrap(),
        Arc::new(Token),
        vec![],
    )
    .with_google_oauth(
        GoogleOAuthRouteConfig::new(
            "google-client",
            "https://app.example/api/reborn/product-auth/oauth/google/callback",
        )
        .unwrap(),
    );
    let app = webui_v2_app(bundle, config).unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/reborn/product-auth/oauth/google/start")
                .header(header::AUTHORIZATION, "Bearer operator-token")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "flow_id":brassclaw_auth::AuthFlowId::new(),
                        "invocation_id":InvocationId::new(),
                        "account_label":"unbacked Google metadata",
                        "scopes":[brassclaw_auth::GOOGLE_CALENDAR_READONLY_SCOPE],
                        "expires_at":chrono::Utc::now()+chrono::Duration::minutes(5),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["code"], "backend_unavailable");
    assert!(value.get("authorization_url").is_none());
}
