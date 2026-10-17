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
        expected_previous_version: None,
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
                "SELECT count(*) FROM reborn_component_review_requests WHERE submission_id=$1",
                &[&id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
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
    #[cfg(feature = "root-llm-provider")]
    {
        let reconcile_uri = format!("{uri}/reconcile");
        let request = json!({"evidence_checksum":view["evidence_checksum"]});
        assert_eq!(
            send(&app, "POST", &reconcile_uri, Some(&request), false)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        let (status, result) = send(&app, "POST", &reconcile_uri, Some(&request), true).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            result["retention_released"], false,
            "store rows cannot replace a missing native owner"
        );
        assert_eq!(result["reconciliation_recorded"], false);
        assert_eq!(result["work_replayed"], false);
        let mut forged = request.clone();
        forged["actual_usage"] = json!(0);
        assert_eq!(
            send(&app, "POST", &reconcile_uri, Some(&forged), true)
                .await
                .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        let stale = json!({"evidence_checksum":"0".repeat(64)});
        assert_eq!(
            send(&app, "POST", &reconcile_uri, Some(&stale), true)
                .await
                .0,
            StatusCode::CONFLICT
        );
    }
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

#[tokio::test]
async fn native_review_lineage_survives_packaged_drafts_and_exact_retries() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let component = Uuid::new_v4();
    let original = draft(component, &[], "result = 'original'");
    let base = store.stage(&original, 0).await.unwrap();
    let package = draft(component, &[], "result = 'packaged successor'");
    let packaged = store.retain_packaged_draft(&package).await.unwrap();
    assert_eq!(packaged.version, 2);
    let api = app(rig.pool.clone());
    let head_uri = format!("/api/webchat/v2/component-revision-heads/{component}");
    assert_eq!(
        send(&api, "GET", &head_uri, None, false).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, head) = send(&api, "GET", &head_uri, None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(head["last_allocated_version"], json!(2));
    assert_eq!(head["class_code"], json!(22));
    let edit = draft(component, &[], "result = 'authored override draft'");
    let submission = Uuid::new_v4();
    let uri = "/api/webchat/v2/component-review-submissions";
    let mut input = request(submission, &edit, Some(base), &[]);
    input["expected_previous_version"] = json!(2);
    let (status, receipt) = send(&api, "POST", uri, Some(&input), true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(receipt["candidate"]["version"], json!(3));
    assert_eq!(receipt["base"]["version"], json!(1));
    assert_eq!(receipt["review_status"], json!("unreviewed"));
    assert_eq!(receipt["catalogue_activated"], json!(false));
    let get_uri = format!("{uri}/{submission}");
    let (status, view) = send(&api, "GET", &get_uri, None, true).await;
    assert_eq!(status, StatusCode::OK);
    let subject: Value = serde_json::from_str(view["subject_bytes"].as_str().unwrap()).unwrap();
    assert_eq!(subject["format"], json!("component-review-submission/2"));
    assert_eq!(subject["expected_previous_version"], json!(2));
    let later = store
        .retain_packaged_draft(&draft(component, &[], "result = 'later package'"))
        .await
        .unwrap();
    assert_eq!(later.version, 4);
    let (status, retry) = send(&api, "POST", uri, Some(&input), true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retry, receipt);
    let (status, again) = send(&api, "GET", &get_uri, None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again, view);
    assert_eq!(
        store
            .read_revision(component, 1)
            .await
            .unwrap()
            .draft()
            .exact_bytes(),
        original.exact_bytes()
    );
    assert_eq!(
        store
            .read_revision(component, 2)
            .await
            .unwrap()
            .draft()
            .exact_bytes(),
        package.exact_bytes()
    );
    let mut stale = input.clone();
    stale["submission_id"] = json!(Uuid::new_v4().to_string());
    assert_eq!(
        send(&api, "POST", uri, Some(&stale), true).await.0,
        StatusCode::CONFLICT
    );
    let mut changed = input.clone();
    changed["expected_previous_version"] = json!(4);
    assert_eq!(
        send(&api, "POST", uri, Some(&changed), true).await.0,
        StatusCode::CONFLICT
    );
    let mut invalid = stale.clone();
    invalid["expected_previous_version"] = json!(0);
    assert_eq!(
        send(&api, "POST", uri, Some(&invalid), true).await.0,
        StatusCode::BAD_REQUEST
    );
    let unknown = Uuid::new_v4();
    let (status, head) = send(
        &api,
        "GET",
        &format!("/api/webchat/v2/component-revision-heads/{unknown}"),
        None,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(head["last_allocated_version"], json!(0));
    assert_eq!(head["class_code"], Value::Null);
    assert_eq!(
        send(
            &api,
            "GET",
            "/api/webchat/v2/component-revision-heads/not-a-uuid",
            None,
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let client = rig.pool.get().await.unwrap();
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM brassclaw_monty_active_catalogue", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_component_review_submissions WHERE component_id=$1",
                &[&component]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(
        store.read_allocation_head(component).await.unwrap(),
        Some((22, 4))
    );
}

#[tokio::test]
async fn native_review_request_is_atomic_immutable_exact_and_backfilled_on_upgrade() {
    use brassclaw_skills::review_submission_store::PgReviewSubmissionStore;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let revisions = PgComponentRevisionStore::new(rig.pool.clone());
    let store = PgReviewSubmissionStore::new(rig.pool.clone());
    let component = Uuid::new_v4();
    let submission = Uuid::new_v4();
    let proposal = ReviewSubmissionDraft::new(
        submission,
        "request-operator",
        draft(component, &[], "result = 'exact request'"),
        None,
        vec![],
    )
    .unwrap();
    let retained = store.submit(&proposal).await.unwrap();
    let again = store.submit(&proposal).await.unwrap();
    assert_eq!(again.subject_bytes, retained.subject_bytes);
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let request = PgReviewSubmissionStore::read_review_request_in_transaction(&tx, submission)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(request.submission_id(), submission);
    assert_eq!(request.subject_checksum(), retained.checksum);
    assert_eq!(
        tx.query_one(
            "SELECT count(*) FROM reborn_component_review_requests WHERE submission_id=$1",
            &[&submission]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        1
    );
    assert!(
        PgReviewSubmissionStore::read_review_request_in_transaction(&tx, Uuid::new_v4())
            .await
            .unwrap()
            .is_none()
    );
    tx.commit().await.unwrap();
    for sql in [
        "UPDATE reborn_component_review_requests SET subject_checksum=repeat('0',64)",
        "DELETE FROM reborn_component_review_requests",
        "TRUNCATE reborn_component_review_requests",
    ] {
        assert!(client.batch_execute(sql).await.is_err());
    }
    // A request insertion failure must roll back revision allocation and subject,
    // including a deferred failure at the actual transaction commit.
    client.batch_execute("CREATE FUNCTION reject_review_request_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'actual review request commit failure' USING ERRCODE='23514'; END $$; CREATE CONSTRAINT TRIGGER review_request_commit_fault AFTER INSERT ON reborn_component_review_requests DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_review_request_commit()").await.unwrap();
    let failed_component = Uuid::new_v4();
    let failed_id = Uuid::new_v4();
    let failed = ReviewSubmissionDraft::new(
        failed_id,
        "request-operator",
        draft(failed_component, &[], "result = 'rolled back'"),
        None,
        vec![],
    )
    .unwrap();
    assert!(store.submit(&failed).await.is_err());
    assert!(store.read(failed_id).await.unwrap().is_none());
    assert!(
        revisions
            .read_allocation_head(failed_component)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_component_review_requests WHERE submission_id=$1",
                &[&failed_id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    client.batch_execute("DROP TRIGGER review_request_commit_fault ON reborn_component_review_requests; DROP FUNCTION reject_review_request_commit()").await.unwrap();
    let recovered = store.submit(&failed).await.unwrap();
    assert_eq!(recovered.candidate.version, 1);
    client.batch_execute("ALTER TABLE reborn_component_review_submissions DISABLE TRIGGER immutable_submission_requests_review").await.unwrap();
    let missing_id = Uuid::new_v4();
    assert!(client.execute("INSERT INTO reborn_component_review_submissions(submission_id,actor,component_id,candidate_version,base_version,subject_bytes,checksum) SELECT $1::uuid,actor,component_id,candidate_version,base_version,replace(subject_bytes,submission_id::text,($1::uuid)::text),encode(sha256(convert_to(replace(subject_bytes,submission_id::text,($1::uuid)::text),'UTF8')),'hex') FROM reborn_component_review_submissions WHERE submission_id=$2",&[&missing_id,&submission]).await.is_err(),
        "the deferred invariant must prevent a committed subject without review work");
    client.batch_execute("ALTER TABLE reborn_component_review_submissions ENABLE TRIGGER immutable_submission_requests_review").await.unwrap();
    assert!(store.read(missing_id).await.unwrap().is_none());

    // Actual V104/V148 subjects upgraded with V149 retain their exact bytes.
    client.batch_execute("CREATE SCHEMA review_request_upgrade; SET search_path TO review_request_upgrade,public").await.unwrap();
    client
        .batch_execute(include_str!(
            "../../brassclaw_pg/migrations/V104__immutable_component_review_submissions.sql"
        ))
        .await
        .unwrap();
    client
        .batch_execute(include_str!(
            "../../brassclaw_pg/migrations/V148__review_lineage_allocation.sql"
        ))
        .await
        .unwrap();
    client.execute("INSERT INTO reborn_component_review_submissions SELECT * FROM public.reborn_component_review_submissions WHERE submission_id=$1",&[&submission]).await.unwrap();
    client
        .batch_execute(include_str!(
            "../../brassclaw_pg/migrations/V149__immutable_component_review_requests.sql"
        ))
        .await
        .unwrap();
    assert_eq!(client.query_one("SELECT subject_bytes FROM reborn_component_review_submissions WHERE submission_id=$1",&[&submission]).await.unwrap().get::<_, String>(0),retained.subject_bytes);
    assert_eq!(client.query_one("SELECT subject_checksum FROM reborn_component_review_requests WHERE submission_id=$1",&[&submission]).await.unwrap().get::<_, String>(0),retained.checksum);
    client.batch_execute("RESET search_path").await.unwrap();
    let tx = client.transaction().await.unwrap();
    assert!(matches!(
        PgReviewSubmissionStore::read_review_request_in_transaction(&tx, submission).await,
        Err(SubmissionStoreError::Revisions(
            RevisionStoreError::SnapshotIsolation
        ))
    ));
    tx.rollback().await.unwrap();
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM brassclaw_monty_active_catalogue", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

#[tokio::test]
async fn native_validator_subject_preserves_complete_pinned_graph_and_base_comparison() {
    use brassclaw_skills::review_submission_store::PgReviewSubmissionStore;
    use brassclaw_skills::value_contract::ContractLimits;

    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let dependency_id = Uuid::new_v4();
    let dependency = draft(dependency_id, &[], "result = 'pinned dependency'");
    let dependency_ref = store.stage(&dependency, 0).await.unwrap();
    let old_dependency_id = Uuid::new_v4();
    store
        .stage(
            &draft(old_dependency_id, &[], "result = 'old base dependency'"),
            0,
        )
        .await
        .unwrap();
    let component = Uuid::new_v4();
    let original = draft(component, &[old_dependency_id], "result = 'base'");
    let base = store.stage(&original, 0).await.unwrap();
    store
        .retain_packaged_draft(&draft(component, &[], "result = 'package allocation'"))
        .await
        .unwrap();
    let source = "result = '__import__(\"os\") {{vars.slot0}} ä猫' # data only";
    let candidate = draft(component, &[dependency_id], source);
    let submission_id = Uuid::new_v4();
    let mut input = request(submission_id, &candidate, Some(base), &[dependency_ref]);
    input["expected_previous_version"] = json!(2);
    assert_eq!(
        send(
            &app(rig.pool.clone()),
            "POST",
            "/api/webchat/v2/component-review-submissions",
            Some(&input),
            true
        )
        .await
        .0,
        StatusCode::OK
    );
    let limits = ContractLimits {
        max_depth: 64,
        max_nodes: 262144,
        max_bytes: 16 * 1024 * 1024,
    };
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let first =
        PgReviewSubmissionStore::read_validator_subject_in_transaction(&tx, submission_id, limits)
            .await
            .unwrap()
            .unwrap();
    let value = first.value();
    assert_eq!(first.candidate().uuid, component);
    assert_eq!(first.candidate().class_code, 22);
    assert_eq!(first.candidate().version, 3);
    assert_eq!(value["candidate"]["reference"]["version"], 3);
    assert_eq!(
        value["candidate"]["revision_bytes"],
        candidate.exact_bytes()
    );
    assert_eq!(
        value["candidate"]["revision"]["document"],
        *candidate.document()
    );
    assert_eq!(
        value["candidate"]["revision"]["document"]["content"],
        source
    );
    assert_eq!(value["base_comparison"]["reference"]["version"], 1);
    assert_eq!(
        value["base_comparison"]["revision_bytes"],
        original.exact_bytes()
    );
    assert_eq!(
        value["base_comparison"]["revision"]["dependencies"],
        json!([old_dependency_id])
    );
    assert_eq!(value["dependencies"].as_array().unwrap().len(), 1);
    assert_eq!(
        value["dependencies"][0]["revision_bytes"],
        dependency.exact_bytes()
    );
    assert!(value.get("passed").is_none());
    assert!(value.get("approved").is_none());
    assert!(value.get("catalogue_activated").is_none());
    store
        .stage(&draft(dependency_id, &[], "result = 'new dependency'"), 1)
        .await
        .unwrap();
    store
        .retain_packaged_draft(&draft(component, &[], "result = 'new package'"))
        .await
        .unwrap();
    let again =
        PgReviewSubmissionStore::read_validator_subject_in_transaction(&tx, submission_id, limits)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(again.value(), first.value());
    assert!(matches!(
        PgReviewSubmissionStore::read_validator_subject_in_transaction(
            &tx,
            submission_id,
            ContractLimits {
                max_bytes: 1,
                ..limits
            }
        )
        .await,
        Err(brassclaw_skills::review_submission_store::SubmissionStoreError::Capacity)
    ));
    assert!(matches!(
        PgReviewSubmissionStore::read_validator_subject_in_transaction(
            &tx,
            submission_id,
            ContractLimits {
                max_depth: 0,
                ..limits
            }
        )
        .await,
        Err(brassclaw_skills::review_submission_store::SubmissionStoreError::Capacity)
    ));
    let mut corrupt_ref = base;
    corrupt_ref.checksum[0] ^= 1;
    assert!(
        PgComponentRevisionStore::read_revision_in_transaction(&tx, corrupt_ref)
            .await
            .is_err()
    );
    let mut missing_ref = base;
    missing_ref.version = 999;
    assert!(
        PgComponentRevisionStore::read_revision_in_transaction(&tx, missing_ref)
            .await
            .is_err()
    );
    tx.commit().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let latest_read =
        PgReviewSubmissionStore::read_validator_subject_in_transaction(&tx, submission_id, limits)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(latest_read.value(), first.value());
    assert!(
        PgReviewSubmissionStore::read_validator_subject_in_transaction(&tx, Uuid::new_v4(), limits)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        tx.query_one("SELECT count(*) FROM brassclaw_monty_active_catalogue", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert_eq!(
        tx.query_one(
            "SELECT count(*) FROM reborn_component_review_submissions",
            &[]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        1
    );
    tx.commit().await.unwrap();
    let tx = client.transaction().await.unwrap();
    assert!(matches!(
        PgReviewSubmissionStore::read_validator_subject_in_transaction(&tx, submission_id, limits)
            .await,
        Err(
            brassclaw_skills::review_submission_store::SubmissionStoreError::Revisions(
                brassclaw_skills::revision_store::RevisionStoreError::SnapshotIsolation
            )
        )
    ));
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn native_review_lineage_upgrade_preserves_v1_and_database_rejection_rolls_back() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let component = Uuid::new_v4();
    let original = draft(component, &[], "result = 'original'");
    let base = store.stage(&original, 0).await.unwrap();
    let old = ReviewSubmissionDraft::new(
        Uuid::new_v4(),
        "migration-operator",
        draft(component, &[], "result = 'legacy edit'"),
        Some(base),
        vec![],
    )
    .unwrap();
    let retained = PgReviewSubmissionStore::new(rig.pool.clone())
        .submit(&old)
        .await
        .unwrap();
    assert_eq!(retained.candidate.version, 2);
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE SCHEMA review_lineage_upgrade; SET search_path TO review_lineage_upgrade,public").await.unwrap();
    client
        .batch_execute(include_str!(
            "../../brassclaw_pg/migrations/V104__immutable_component_review_submissions.sql"
        ))
        .await
        .unwrap();
    client.execute("INSERT INTO reborn_component_review_submissions SELECT * FROM public.reborn_component_review_submissions WHERE submission_id=$1",&[&retained.id]).await.unwrap();
    client
        .batch_execute(include_str!(
            "../../brassclaw_pg/migrations/V148__review_lineage_allocation.sql"
        ))
        .await
        .unwrap();
    let bytes: String = client
        .query_one(
            "SELECT subject_bytes FROM reborn_component_review_submissions WHERE submission_id=$1",
            &[&retained.id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(bytes, retained.subject_bytes);
    client.batch_execute("RESET search_path; CREATE FUNCTION reject_new_review_subject() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'deliberate review retention failure'; END $$; CREATE TRIGGER reject_new_review_subject BEFORE INSERT ON reborn_component_review_submissions FOR EACH ROW EXECUTE FUNCTION reject_new_review_subject()").await.unwrap();
    let candidate = draft(component, &[], "result = 'new lineage'");
    let draft = ReviewSubmissionDraft::new_after_head(
        Uuid::new_v4(),
        "migration-operator",
        candidate,
        Some(base),
        vec![],
        2,
    )
    .unwrap();
    assert!(
        PgReviewSubmissionStore::new(rig.pool.clone())
            .submit(&draft)
            .await
            .is_err()
    );
    assert_eq!(
        store.read_allocation_head(component).await.unwrap(),
        Some((22, 2))
    );
    assert!(store.read_revision(component, 3).await.is_err());
    client.batch_execute("DROP TRIGGER reject_new_review_subject ON reborn_component_review_submissions; DROP FUNCTION reject_new_review_subject()").await.unwrap();
    let submitted = PgReviewSubmissionStore::new(rig.pool.clone())
        .submit(&draft)
        .await
        .unwrap();
    assert_eq!(submitted.candidate.version, 3);
    assert_eq!(submitted.base, Some(base));
    assert_eq!(
        PgReviewSubmissionStore::new(rig.pool.clone())
            .read(retained.id)
            .await
            .unwrap()
            .unwrap()
            .subject_bytes,
        retained.subject_bytes
    );
    // SQL cannot accept a v2 subject whose counter does not explain the actual
    // allocated version, even if its checksum was recomputed by a DB caller.
    let mut bad: Value = serde_json::from_str(&submitted.subject_bytes).unwrap();
    let bad_id = Uuid::new_v4();
    bad["submission_id"] = json!(bad_id.to_string());
    bad["expected_previous_version"] = json!(1);
    let bad = bad.to_string();
    assert!(client.execute("INSERT INTO reborn_component_review_submissions(submission_id,actor,component_id,candidate_version,base_version,subject_bytes,checksum) VALUES($1,$2,$3,3,1,$4,encode(sha256(convert_to($4,'UTF8')),'hex'))",&[&bad_id,&"migration-operator",&component,&bad]).await.is_err());
}
