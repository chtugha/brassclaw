//! Native HTTP caller acceptance reuses actual Monty/kernel/Q1/behavior
//! observations. No passing evidence or external Tool result is simulated.

use crate::{RebornReadiness, WebuiAuthenticator, WebuiServeConfig, webui_v2_app};
use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use brassclaw_host_api::{TenantId, UserId};
use brassclaw_product_workflow::RebornServices;
use brassclaw_threads::PgSessionThreadService;
use brassclaw_turns::{DefaultTurnCoordinator, PgTurnStateStore};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

#[path = "../../../tests/monty_control/tests/support/q1_review_fixture.rs"]
mod native_evidence;

struct ReviewOperator;
#[async_trait]
impl WebuiAuthenticator for ReviewOperator {
    async fn authenticate(&self, token: &str) -> Option<UserId> {
        (token == "native-review-bearer").then(|| UserId::new("native-review-operator").unwrap())
    }
    fn allows_operator_webui_config(&self) -> bool {
        true
    }
}

async fn post(app: &Router, uri: &str, body: &Value, authenticated: bool) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("Content-Type", "application/json");
    if authenticated {
        request = request.header("Authorization", "Bearer native-review-bearer");
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

struct HttpReviewObserver;
#[async_trait]
impl native_evidence::ReviewObserver for HttpReviewObserver {
    async fn review(
        &self,
        pool: Arc<brassclaw_pg::PgPool>,
        skill: Uuid,
        q1: Uuid,
        behavior: Uuid,
        failed: bool,
    ) {
        let tenant = "native-review-instance";
        let api =
            RebornServices::new(
                Arc::new(PgSessionThreadService::new(pool.clone(), tenant)),
                Arc::new(DefaultTurnCoordinator::new(Arc::new(
                    PgTurnStateStore::new(pool.clone(), tenant),
                ))),
            )
            .with_recipe_store(Arc::new(
                crate::pg_recipe_store::PgRecipeStoreFacade::new(pool.clone(), tenant, "default"),
            ));
        let app = webui_v2_app(
            crate::RebornWebuiBundle {
                api: Arc::new(api),
                product_auth: None,
                readiness: RebornReadiness::disabled(),
            },
            WebuiServeConfig::new(
                TenantId::new(tenant).unwrap(),
                Arc::new(ReviewOperator),
                Vec::new(),
            ),
        )
        .unwrap();
        let preview_uri = format!("/api/webchat/v2/skills/{skill}/association-review");
        let approval_uri = format!("/api/webchat/v2/skills/{skill}/association-approval");
        let selection = json!({"q1_ref":q1,"behavioral_refs":[behavior]});
        assert_eq!(
            post(&app, &preview_uri, &selection, false).await.0,
            StatusCode::UNAUTHORIZED
        );
        let (status, view) = post(&app, &preview_uri, &selection, true).await;
        if failed {
            assert_eq!(status, StatusCode::BAD_REQUEST);
            return;
        }
        assert_eq!(status, StatusCode::OK);
        assert_eq!(view["review"]["skill_uuid"], skill.to_string());
        assert_eq!(view["review"]["components"].as_array().unwrap().len(), 4);
        assert_eq!(view["review"]["evidence"].as_array().unwrap().len(), 2);
        let approval = Uuid::new_v4();
        let request = json!({"approval_id":approval,"selection":selection,
            "review_checksum":view["review_checksum"],
            "semantic_review":"Reviewed the retained JSON usage prose, Python input binding and actual kernel outcomes."});
        assert_eq!(
            post(&app, &approval_uri, &request, false).await.0,
            StatusCode::UNAUTHORIZED
        );
        let mut changed = request.clone();
        changed["actor"] = json!("forged-operator");
        assert_eq!(
            post(&app, &approval_uri, &changed, true).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        changed = request.clone();
        changed["review_checksum"] = json!("changed-view");
        assert_eq!(
            post(&app, &approval_uri, &changed, true).await.0,
            StatusCode::BAD_REQUEST
        );
        let (status, receipt) = post(&app, &approval_uri, &request, true).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(receipt["approval_id"], approval.to_string());
        assert_eq!(receipt["catalogue_activated"], false);
        let (status, retry) = post(&app, &approval_uri, &request, true).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(receipt, retry);
        let client = pool.get().await.unwrap();
        let q2 = Uuid::parse_str(receipt["q2_ref"].as_str().unwrap()).unwrap();
        let bytes: String = client
            .query_one(
                "SELECT evidence_bytes FROM reborn_component_review_evidence WHERE evidence_id=$1",
                &[&q2],
            )
            .await
            .unwrap()
            .get(0);
        let record: Value = serde_json::from_str(&bytes).unwrap();
        assert_eq!(record["report"]["actor"], "native-review-operator");
        assert_eq!(
            record["report"]["semantic_review"],
            request["semantic_review"]
        );
        assert_eq!(record["report"]["review_checksum"], view["review_checksum"]);
        let rows: i64 = client
            .query_one(
                "SELECT count(*) FROM reborn_skill_association_approvals WHERE approval_id=$1",
                &[&approval],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(rows, 1);
    }
}

#[tokio::test]
async fn native_http_human_review_pins_actual_observations_and_authenticated_actor() {
    native_evidence::exercise_actual_behavior(Some(&HttpReviewObserver)).await;
}
