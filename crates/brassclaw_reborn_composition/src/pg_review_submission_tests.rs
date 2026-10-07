//! Real PostgreSQL and the mounted operator HTTP path. No review pass, Tool
//! execution, model answer or activation is manufactured by these fixtures.
use super::*;
use crate::{RebornReadiness, WebuiAuthenticator, WebuiServeConfig, webui_v2_app};
use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use brassclaw_host_api::{TenantId, UserId};
use brassclaw_product_workflow::RebornServices;
use brassclaw_skills::revision_store::PgComponentRevisionStore;
use brassclaw_threads::PgSessionThreadService;
use brassclaw_turns::{DefaultTurnCoordinator, PgTurnStateStore};
use serde_json::{Value, json};
use tokio_postgres::IsolationLevel;
use tower::ServiceExt;

struct Operator;
#[async_trait]
impl WebuiAuthenticator for Operator {
    async fn authenticate(&self, token: &str) -> Option<UserId> {
        (token == "submission-test-bearer")
            .then(|| UserId::new("submission-test-operator").unwrap())
    }
    fn allows_operator_webui_config(&self) -> bool {
        true
    }
}
fn draft(id: Uuid, dependencies: &[Uuid], content: &str) -> ComponentRevisionDraft {
    ComponentRevisionDraft::from_json(
        &json!({
            "format":"component-revision/1", "uuid":id, "class_code":22,
            "document":{"content":content, "includes":[], "dependency_registry":null,
                "input_schema":{"type":"object","properties":{}},
                "result_schema":{"type":"string"}, "override_prompt_creation":true},
            "dependencies":dependencies, "association":null,
        })
        .to_string(),
    )
    .unwrap()
}
fn request(
    id: Uuid,
    draft: &ComponentRevisionDraft,
    base: Option<ComponentRevisionRef>,
    deps: &[ComponentRevisionRef],
) -> Value {
    serde_json::to_value(SubmitComponentReviewRequest {
        submission_id: id.to_string(),
        candidate_bytes: draft.exact_bytes().into(),
        base: base.map(selection),
        dependencies: deps.iter().copied().map(selection).collect(),
    })
    .unwrap()
}
async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<&Value>,
    token: bool,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if token {
        builder = builder.header("Authorization", "Bearer submission-test-bearer");
    }
    let content = match body {
        Some(value) => {
            builder = builder.header("Content-Type", "application/json");
            value.to_string()
        }
        None => String::new(),
    };
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(content)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 80 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
fn app(pool: Arc<PgPool>) -> Router {
    let tenant = "submission-test-instance";
    let api = RebornServices::new(
        Arc::new(PgSessionThreadService::new(pool.clone(), tenant)),
        Arc::new(DefaultTurnCoordinator::new(Arc::new(
            PgTurnStateStore::new(pool.clone(), tenant),
        ))),
    )
    .with_recipe_store(Arc::new(crate::pg_recipe_store::PgRecipeStoreFacade::new(
        pool, tenant, "default",
    )));
    webui_v2_app(
        crate::RebornWebuiBundle {
            api: Arc::new(api),
            product_auth: None,
            readiness: RebornReadiness::disabled(),
        },
        WebuiServeConfig::new(
            TenantId::new(tenant).unwrap(),
            Arc::new(Operator),
            Vec::new(),
        ),
    )
    .unwrap()
}

#[tokio::test]
async fn native_http_submission_retains_exact_graph_and_recovers_commit_without_reallocating() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let dependency_id = Uuid::new_v4();
    let dependency = draft(dependency_id, &[], "result = 'original dependency'");
    let pinned = store.stage(&dependency, 0).await.unwrap();
    let component = Uuid::new_v4();
    let source = "result = 'quotes \\\" {braces} ä猫'; __import__('os') # candidate data only";
    let original = draft(component, &[dependency_id], source);
    let submission = Uuid::new_v4();
    let input = request(submission, &original, None, &[pinned]);
    let app = app(rig.pool.clone());
    let uri = "/api/webchat/v2/component-review-submissions";
    let read_uri = format!("{uri}/{submission}");
    assert_eq!(
        send(&app, "POST", uri, Some(&input), false).await.0,
        StatusCode::UNAUTHORIZED
    );
    let mut forged = input.clone();
    forged["actor"] = json!("forged");
    assert_eq!(
        send(&app, "POST", uri, Some(&forged), true).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (status, first) = send(&app, "POST", uri, Some(&input), true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["review_status"], "unreviewed");
    assert_eq!(first["catalogue_activated"], false);
    assert_eq!(first["candidate"]["version"], 1);
    let root: ComponentRevisionSelection =
        serde_json::from_value(first["candidate"].clone()).unwrap();
    let root = reference(&root).unwrap();
    // A dependency head and the candidate head advance; retry keeps the old
    // subject, source and selection rather than allocating another version.
    store
        .stage(&draft(dependency_id, &[], "result = 'replacement'"), 1)
        .await
        .unwrap();
    let changed = draft(component, &[dependency_id], "result = 'new candidate'");
    let next_input = request(Uuid::new_v4(), &changed, Some(root), &[pinned]);
    assert_eq!(
        send(&app, "POST", uri, Some(&next_input), true).await.0,
        StatusCode::OK
    );
    let (status, recovered) = send(&app, "POST", uri, Some(&input), true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(recovered, first);
    assert_eq!(
        send(&app, "GET", &read_uri, None, false).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(&app, "HEAD", &read_uri, None, false).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, view) = send(&app, "GET", &read_uri, None, true).await;
    assert_eq!(status, StatusCode::OK);
    let subject: Value = serde_json::from_str(view["subject_bytes"].as_str().unwrap()).unwrap();
    assert_eq!(subject["actor"], "submission-test-operator");
    assert_eq!(subject["candidate_bytes"], original.exact_bytes());
    assert_eq!(view["component_bytes"].as_array().unwrap().len(), 2);
    assert!(
        view["component_bytes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str() == Some(dependency.exact_bytes()))
    );
    let mut changed_id = input.clone();
    changed_id["candidate_bytes"] = json!(changed.exact_bytes());
    assert_eq!(
        send(&app, "POST", uri, Some(&changed_id), true).await.0,
        StatusCode::CONFLICT
    );
    // The exact base checksum, not just the number, is required.
    let mut wrong_base = next_input.clone();
    wrong_base["submission_id"] = json!(Uuid::new_v4());
    wrong_base["base"]["checksum"] = json!("00".repeat(32));
    assert_eq!(
        send(&app, "POST", uri, Some(&wrong_base), true).await.0,
        StatusCode::CONFLICT
    );
    let client = rig.pool.get().await.unwrap();
    let last: i64 = client
        .query_one(
            "SELECT last_version FROM reborn_component_revision_heads WHERE component_id=$1",
            &[&component],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(last, 2);
    for sql in [
        "UPDATE reborn_component_review_submissions SET actor='replacement'",
        "DELETE FROM reborn_component_review_submissions",
        "TRUNCATE reborn_component_review_submissions",
    ] {
        assert!(
            client.batch_execute(sql).await.is_err(),
            "subjects remain immutable"
        );
    }
    let evidence: i64 = client.query_one("SELECT (SELECT count(*) FROM reborn_component_review_evidence) + (SELECT count(*) FROM reborn_skill_association_approvals)", &[]).await.unwrap().get(0);
    assert_eq!(
        evidence, 0,
        "retention produces no successful review evidence"
    );
    // Authoring payloads above Json's usual 2 MiB limit reach the actual route.
    let large = draft(Uuid::new_v4(), &[], &"猫".repeat(800_000));
    assert_eq!(
        send(
            &app,
            "POST",
            uri,
            Some(&request(Uuid::new_v4(), &large, None, &[])),
            true
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn native_submission_rolls_back_incomplete_graph_and_failed_commit_and_fences_concurrent_edits()
 {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let store = PgReviewSubmissionStore::new(rig.pool.clone());
    let root = Uuid::new_v4();
    let missing = ReviewSubmissionDraft::new(
        Uuid::new_v4(),
        "author",
        draft(root, &[Uuid::new_v4()], "result = 1"),
        None,
        Vec::new(),
    )
    .unwrap();
    assert!(store.submit(&missing).await.is_err());
    let mut client = rig.pool.get().await.unwrap();
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_component_revision_heads WHERE component_id=$1",
                &[&root]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    client.batch_execute("CREATE FUNCTION fail_submission_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'actual submission commit failure' USING ERRCODE='23514'; END; $$; CREATE CONSTRAINT TRIGGER submission_commit_fault AFTER INSERT ON reborn_component_review_submissions DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_submission_commit()").await.unwrap();
    let id = Uuid::new_v4();
    let initial = ReviewSubmissionDraft::new(
        id,
        "author",
        draft(root, &[], "result = 1"),
        None,
        Vec::new(),
    )
    .unwrap();
    assert!(matches!(
        store.submit(&initial).await,
        Err(SubmissionStoreError::Database(_))
    ));
    assert!(store.read(id).await.unwrap().is_none());
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_component_revision_heads WHERE component_id=$1",
                &[&root]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    client.batch_execute("DROP TRIGGER submission_commit_fault ON reborn_component_review_submissions; DROP FUNCTION fail_submission_commit()").await.unwrap();
    let retained = store.submit(&initial).await.unwrap();
    assert_eq!(retained.candidate.version, 1);
    let forged = ReviewSubmissionDraft::new(
        id,
        "other author",
        draft(root, &[], "result = 1"),
        None,
        Vec::new(),
    )
    .unwrap();
    assert!(matches!(
        store.submit(&forged).await,
        Err(SubmissionStoreError::Conflict)
    ));
    let left = ReviewSubmissionDraft::new(
        Uuid::new_v4(),
        "author",
        draft(root, &[], "result = 'left'"),
        Some(retained.candidate),
        Vec::new(),
    )
    .unwrap();
    let right = ReviewSubmissionDraft::new(
        Uuid::new_v4(),
        "author",
        draft(root, &[], "result = 'right'"),
        Some(retained.candidate),
        Vec::new(),
    )
    .unwrap();
    let (left_result, right_result) = tokio::join!(store.submit(&left), store.submit(&right));
    assert_ne!(
        left_result.is_ok(),
        right_result.is_ok(),
        "exactly one base-CAS edit wins"
    );
    assert_eq!(
        store.submit(&initial).await.unwrap().candidate,
        retained.candidate
    );
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::ReadCommitted)
        .start()
        .await
        .unwrap();
    assert!(matches!(
        PgReviewSubmissionStore::submit_in_transaction(&tx, &initial).await,
        Err(SubmissionStoreError::Revisions(
            RevisionStoreError::SnapshotIsolation
        ))
    ));
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn native_sempai_intent_proposal_does_not_slice_inside_unicode() {
    use brassclaw_interceptor::SempaiProposalSink;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let sink = crate::sempai_proposal_sink::PgSempaiProposalSink::new(
        rig.pool.clone(),
        "unicode-instance",
        "default",
    );
    let input = format!("a{}", "猫".repeat(1500));
    let result = sink
        .submit_proposals("author", "default", &[], &[json!({"input":input})], &[])
        .await
        .unwrap();
    assert_eq!(result.intent_examples_queued, 1);
    let client = rig.pool.get().await.unwrap();
    let row = client.query_one("SELECT name,intent_examples,validation_status FROM reborn_recipes WHERE source='sempai_intent_proposal'", &[]).await.unwrap();
    let name: String = row.get(0);
    let suffix = name.strip_prefix("intent-proposal-").unwrap();
    assert_eq!(Uuid::parse_str(suffix).unwrap().to_string(), suffix);
    assert!(name.len() <= 64);
    assert_eq!(row.get::<_, Value>(1), json!([{"input":input}]));
    assert_eq!(row.get::<_, String>(2), "pending");
}
