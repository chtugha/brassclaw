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

// Synthetic stopped journal fixture: this tests management, not model behavior
// or the producer's completeness/approval contract.
#[tokio::test]
async fn native_http_review_inspection_preserves_quarantine_and_exact_observations() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let client = rig.pool.get().await.unwrap();
    let source = Uuid::new_v4();
    let attempt = Uuid::new_v4();
    let event = json!({"format":"monty-completed-turn-review/1","run_id":source,
        "evidence_complete":false,"original":"private quotes \" ä猫"})
    .to_string();
    client.execute("INSERT INTO brassclaw_monty_task_admissions
        (run_id,turn_id,scope,accepted_message_ref,runner_id,claim_checksum,admission_key,phase)
        VALUES($1,$2,'{}','fixture',$3,decode(repeat('01',32),'hex'),decode(repeat('02',32),'hex'),'reserved')",
        &[&source,&Uuid::new_v4(),&Uuid::new_v4()]).await.unwrap();
    client
        .execute(
            "INSERT INTO brassclaw_monty_review_events(run_id,event_bytes,event_checksum)
        VALUES($1,$2,encode(sha256(convert_to($2,'UTF8')),'hex'))",
            &[&source, &event],
        )
        .await
        .unwrap();
    client.execute("INSERT INTO brassclaw_monty_review_work
        (source_run_id,attempt_id,owner_id,event_checksum,selection_bytes,prefix_bytes,model_identity)
        SELECT $1,$2,$3,event_checksum,'immutable selection','pinned prefix','test model'
        FROM brassclaw_monty_review_events WHERE run_id=$1", &[&source,&attempt,&Uuid::new_v4()]).await.unwrap();
    let app = app(rig.pool.clone());
    let list_uri = "/api/webchat/v2/post-turn-reviews";
    let uri = format!("{list_uri}/{attempt}");
    let write_uri = format!("{uri}/dispositions");
    assert_eq!(
        send(&app, "GET", list_uri, None, false).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(&app, "GET", &uri, None, false).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, listed) = send(&app, "GET", list_uri, None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["items"][0]["attempt_id"], attempt.to_string());
    assert_eq!(listed["items"][0]["settlement_recorded"], false);
    let (status, view) = send(&app, "GET", &uri, None, true).await;
    assert_eq!(status, StatusCode::OK);
    let disposition = Uuid::new_v4();
    let mut input = json!({"disposition_id":disposition,"evidence_checksum":view["evidence_checksum"],
        "note":"Unknown provider effects; keep retention. \" ä猫"});
    assert_eq!(
        send(&app, "POST", &write_uri, Some(&input), true).await.0,
        StatusCode::CONFLICT,
        "live work cannot accept a stopped-review observation"
    );
    client
        .execute(
            "UPDATE brassclaw_monty_review_work SET phase='uncertain' WHERE attempt_id=$1",
            &[&attempt],
        )
        .await
        .unwrap();
    client.execute("INSERT INTO brassclaw_monty_review_operations(attempt_id,operation,input_bytes,input_checksum)
        VALUES($1,'ask_sempai','{}',encode(sha256(convert_to('{}','UTF8')),'hex'))", &[&attempt]).await.unwrap();
    let (status, before) = send(&app, "GET", &uri, None, true).await;
    assert_eq!(status, StatusCode::OK);
    let envelope: Value = serde_json::from_str(before["evidence_bytes"].as_str().unwrap()).unwrap();
    assert_eq!(envelope["journal_integrity"], true);
    assert_eq!(envelope["event"]["event_bytes"], event);
    assert_eq!(envelope["operations"][0]["output_bytes"], Value::Null);
    input["evidence_checksum"] = before["evidence_checksum"].clone();
    // A late actual operation answer changes the observation, without resuming
    // the stopped task or changing its immutable terminal phase.
    client.execute("UPDATE brassclaw_monty_review_operations SET output_bytes='late observed response',
        output_checksum=encode(sha256(convert_to('late observed response','UTF8')),'hex'),answered_at=clock_timestamp()
        WHERE attempt_id=$1", &[&attempt]).await.unwrap();
    assert_eq!(
        send(&app, "POST", &write_uri, Some(&input), true).await.0,
        StatusCode::CONFLICT
    );
    let (status, refreshed) = send(&app, "GET", &uri, None, true).await;
    assert_eq!(status, StatusCode::OK);
    input["evidence_checksum"] = refreshed["evidence_checksum"].clone();
    assert_eq!(
        send(&app, "POST", &write_uri, Some(&input), false).await.0,
        StatusCode::UNAUTHORIZED
    );
    client.batch_execute("CREATE FUNCTION fail_observation_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'private note must not leak' USING ERRCODE='23514'; END; $$; CREATE CONSTRAINT TRIGGER observation_commit_fault AFTER INSERT ON brassclaw_monty_review_dispositions DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_observation_commit()").await.unwrap();
    let (status, failure) = send(&app, "POST", &write_uri, Some(&input), true).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(!failure.to_string().contains("private note must not leak"));
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_review_dispositions",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    client.batch_execute("DROP TRIGGER observation_commit_fault ON brassclaw_monty_review_dispositions; DROP FUNCTION fail_observation_commit()").await.unwrap();
    let (status, receipt) = send(&app, "POST", &write_uri, Some(&input), true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(receipt["retention_released"], false);
    assert_eq!(receipt["work_replayed"], false);
    assert_eq!(
        send(&app, "POST", &write_uri, Some(&input), true).await,
        (StatusCode::OK, receipt.clone())
    );
    let mut changed = input.clone();
    changed["note"] = json!("replacement observation");
    assert_eq!(
        send(&app, "POST", &write_uri, Some(&changed), true).await.0,
        StatusCode::CONFLICT
    );
    let mut forged = input.clone();
    forged["actor"] = json!("forged");
    assert_eq!(
        send(&app, "POST", &write_uri, Some(&forged), true).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (_, observed) = send(&app, "GET", &uri, None, true).await;
    assert_eq!(observed["disposition_count"], 1);
    assert_eq!(
        observed["dispositions"][0]["actor"],
        "submission-test-operator"
    );
    assert_eq!(observed["dispositions"][0]["note"], input["note"]);
    let retained=client.query_one("SELECT evidence_bytes FROM brassclaw_monty_review_dispositions WHERE disposition_id=$1", &[&disposition]).await.unwrap();
    assert_eq!(
        retained.get::<_, String>(0),
        refreshed["evidence_bytes"].as_str().unwrap()
    );
    for sql in [
        "UPDATE brassclaw_monty_review_dispositions SET note='changed'",
        "DELETE FROM brassclaw_monty_review_dispositions",
        "TRUNCATE brassclaw_monty_review_dispositions",
    ] {
        assert!(client.batch_execute(sql).await.is_err(), "{sql}");
    }
    assert_eq!(client.query_one("SELECT phase,model_dispatch_count FROM brassclaw_monty_review_work WHERE attempt_id=$1", &[&attempt]).await.unwrap().get::<_,String>(0),"uncertain");
    assert_eq!(
        client
            .query_one(
                "SELECT model_dispatch_count FROM brassclaw_monty_review_work WHERE attempt_id=$1",
                &[&attempt]
            )
            .await
            .unwrap()
            .get::<_, i32>(0),
        0
    );
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_review_settlements",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert_eq!(
        send(
            &app,
            "GET",
            &format!("{list_uri}?after={attempt}"),
            None,
            true
        )
        .await
        .1["items"],
        json!([])
    );
    assert_eq!(
        send(
            &app,
            "GET",
            &format!("{list_uri}/{}", Uuid::new_v4()),
            None,
            true
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(
            &app,
            "GET",
            &format!("{list_uri}?after={}", Uuid::nil()),
            None,
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}
