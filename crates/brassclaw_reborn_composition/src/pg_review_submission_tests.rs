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
pub(super) fn draft(id: Uuid, dependencies: &[Uuid], content: &str) -> ComponentRevisionDraft {
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
pub(super) fn request(
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
        function_partner: None,
    })
    .unwrap()
}
pub(super) async fn send(
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
pub(super) fn app(pool: Arc<PgPool>) -> Router {
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
    assert_eq!(first.submission_id(), submission_id);
    assert_eq!(
        value["subject_checksum"].as_str(),
        Some(first.subject_checksum())
    );
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

fn historical_class_draft(id: Uuid, class_code: i32, text: &str) -> ComponentRevisionDraft {
    ComponentRevisionDraft::from_json(
        &json!({
            "format":"component-revision/1","uuid":id,"class_code":class_code,
            "document":{"body":text},"dependencies":[],"association":null,
        })
        .to_string(),
    )
    .unwrap()
}
async fn admission_counts(
    pool: &Arc<PgPool>,
    component: Uuid,
    submission: Uuid,
) -> (i64, i64, i64, i64) {
    let client = pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT
        (SELECT count(*) FROM reborn_component_revision_heads WHERE component_id=$1),
        (SELECT count(*) FROM reborn_component_revisions WHERE component_id=$1),
        (SELECT count(*) FROM reborn_component_review_submissions WHERE submission_id=$2),
        (SELECT count(*) FROM reborn_component_review_requests WHERE submission_id=$2)",
            &[&component, &submission],
        )
        .await
        .unwrap();
    (row.get(0), row.get(1), row.get(2), row.get(3))
}

#[tokio::test]
async fn native_fresh_class50_review_rejects_every_store_admission_before_effects() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let reviews = PgReviewSubmissionStore::new(rig.pool.clone());
    for mode in ["submit", "transaction", "retained"] {
        let component = Uuid::new_v4();
        let submission = Uuid::new_v4();
        let candidate = historical_class_draft(component, 50, "new Scaffold review is retired");
        // Generic exact revision retention remains available for historical
        // bytes. Attaching a NEW review to it must not bypass admission.
        let expected = if mode == "retained" {
            PgComponentRevisionStore::new(rig.pool.clone())
                .stage(&candidate, 0)
                .await
                .unwrap();
            (1, 1, 0, 0)
        } else {
            (0, 0, 0, 0)
        };
        let proposal = if mode == "transaction" {
            ReviewSubmissionDraft::new_after_head(submission, "author", candidate, None, vec![], 0)
                .unwrap()
        } else {
            ReviewSubmissionDraft::new(submission, "author", candidate, None, vec![]).unwrap()
        };
        let result = if mode == "submit" {
            reviews.submit(&proposal).await
        } else if mode == "retained" {
            reviews.submit_retained(&proposal).await
        } else {
            let mut client = rig.pool.get().await.unwrap();
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .start()
                .await
                .unwrap();
            let result = PgReviewSubmissionStore::submit_in_transaction(&tx, &proposal).await;
            tx.rollback().await.unwrap();
            result
        };
        assert!(
            matches!(result, Err(SubmissionStoreError::RetiredScaffold)),
            "fresh admission {mode}"
        );
        assert_eq!(
            admission_counts(&rig.pool, component, submission).await,
            expected
        );
    }
    // The exact guard does not create a blanket protected-root/old-parser ban.
    let component = Uuid::new_v4();
    let submission = Uuid::new_v4();
    let root = ReviewSubmissionDraft::new(
        submission,
        "author",
        historical_class_draft(component, 10, "protected root review data"),
        None,
        vec![],
    )
    .unwrap();
    let retained = reviews.submit(&root).await.unwrap();
    assert_eq!(retained.candidate.class_code, 10);
    assert_eq!(
        admission_counts(&rig.pool, component, submission).await,
        (1, 1, 1, 1)
    );
}

// Immutable historical-format bytes, with the exact old Subject/Reference field
// order. This is test history inserted with normal integrity/FK/V149 triggers,
// NOT a production exception or a way to invoke the retired admission path.
const HISTORICAL_CLASS50_SUBJECT: &str = include_str!("testdata/class50_review_submission_v1.json");
const HISTORICAL_CLASS50_SUBJECT_V2: &str =
    include_str!("testdata/class50_review_submission_v2.json");
async fn retain_historical_class50_fixture(
    pool: &Arc<PgPool>,
    bytes: &str,
) -> RetainedReviewSubmission {
    use sha2::Digest as _;
    let subject: Value = serde_json::from_str(bytes).unwrap();
    let component =
        ComponentRevisionDraft::from_json(subject["candidate_bytes"].as_str().unwrap()).unwrap();
    assert_eq!(component.class_code(), 50);
    let reference = PgComponentRevisionStore::new(pool.clone())
        .stage(&component, 0)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(selection(reference)).unwrap(),
        subject["candidate"]
    );
    let id = Uuid::parse_str(subject["submission_id"].as_str().unwrap()).unwrap();
    let actor = subject["actor"].as_str().unwrap();
    let checksum = hex::encode(sha2::Sha256::digest(bytes.as_bytes()));
    let client = pool.get().await.unwrap();
    client
        .execute(
            "INSERT INTO reborn_component_review_submissions
        (submission_id,actor,component_id,candidate_version,base_version,subject_bytes,checksum)
        VALUES($1,$2,$3,$4,NULL,$5,$6)",
            &[
                &id,
                &actor,
                &component.uuid(),
                &(reference.version as i64),
                &bytes,
                &checksum,
            ],
        )
        .await
        .unwrap();
    drop(client);
    PgReviewSubmissionStore::new(pool.clone())
        .read(id)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn native_historical_class50_originals_pins_and_identical_retries_remain_exact() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    for bytes in [HISTORICAL_CLASS50_SUBJECT, HISTORICAL_CLASS50_SUBJECT_V2] {
        let original = retain_historical_class50_fixture(&rig.pool, bytes).await;
        let store = PgComponentRevisionStore::new(rig.pool.clone());
        let reviews = PgReviewSubmissionStore::new(rig.pool.clone());
        let revision = store
            .read_revision(original.candidate.uuid, original.candidate.version)
            .await
            .unwrap();
        let exact = revision.draft().exact_bytes().to_owned();
        assert_eq!(original.subject_bytes, bytes);
        let graph = store
            .read_exact(&[original.candidate.uuid], &[original.candidate])
            .await
            .unwrap();
        assert_eq!(
            graph.revisions()[&original.candidate.uuid]
                .draft()
                .exact_bytes(),
            exact
        );
        let legacy: Value = serde_json::from_str(bytes).unwrap();
        let same = if legacy["format"] == "component-review-submission/2" {
            ReviewSubmissionDraft::new_after_head(
                original.id,
                &original.actor,
                revision.draft().clone(),
                None,
                vec![],
                0,
            )
            .unwrap()
        } else {
            ReviewSubmissionDraft::new(
                original.id,
                &original.actor,
                revision.draft().clone(),
                None,
                vec![],
            )
            .unwrap()
        };
        for result in [
            reviews.submit(&same).await,
            reviews.submit_retained(&same).await,
        ] {
            let recovered = result.unwrap();
            assert_eq!(recovered.subject_bytes, bytes);
            assert_eq!(recovered.checksum, original.checksum);
            assert_eq!(recovered.candidate, original.candidate);
        }
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        let recovered = PgReviewSubmissionStore::submit_in_transaction(&tx, &same)
            .await
            .unwrap();
        assert_eq!(recovered.subject_bytes, bytes);
        tx.commit().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        let subject = PgReviewSubmissionStore::read_validator_subject_in_transaction(
            &tx,
            original.id,
            brassclaw_skills::component_revision::REVISION_LIMITS,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(subject.candidate(), original.candidate);
        assert_eq!(subject.value()["candidate"]["revision_bytes"], exact);
        tx.commit().await.unwrap();
        let changed = ReviewSubmissionDraft::new(
            original.id,
            "different actor",
            revision.draft().clone(),
            None,
            vec![],
        )
        .unwrap();
        assert!(matches!(
            reviews.submit(&changed).await,
            Err(SubmissionStoreError::Conflict)
        ));
        let upgrade_id = Uuid::new_v4();
        let upgrade = ReviewSubmissionDraft::new_after_head(
            upgrade_id,
            &original.actor,
            historical_class_draft(original.candidate.uuid, 50, "new revision forbidden"),
            Some(original.candidate),
            vec![],
            1,
        )
        .unwrap();
        assert!(matches!(
            reviews.submit(&upgrade).await,
            Err(SubmissionStoreError::RetiredScaffold)
        ));
        assert_eq!(
            store
                .read_allocation_head(original.candidate.uuid)
                .await
                .unwrap(),
            Some((50, 1))
        );
        assert!(
            store
                .read_revision(original.candidate.uuid, 2)
                .await
                .is_err()
        );
        assert_eq!(
            admission_counts(&rig.pool, original.candidate.uuid, original.id).await,
            (1, 1, 1, 1)
        );
        assert_eq!(
            admission_counts(&rig.pool, original.candidate.uuid, upgrade_id).await,
            (1, 1, 0, 0)
        );
        assert_eq!(
            reviews
                .read(original.id)
                .await
                .unwrap()
                .unwrap()
                .subject_bytes,
            bytes
        );
    }
}

#[tokio::test]
async fn native_http_class50_admission_is_visible_and_historical_readback_stays_available() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let api = app(rig.pool.clone());
    let uri = "/api/webchat/v2/component-review-submissions";
    let component = Uuid::new_v4();
    let submission = Uuid::new_v4();
    let candidate = historical_class_draft(component, 50, "new Scaffold");
    let (status, body) = send(
        &api,
        "POST",
        uri,
        Some(&request(submission, &candidate, None, &[])),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // Existing HTTP error vocabulary deliberately redacts backend messages.
    assert_eq!(body["error"], "invalid_request");
    assert_eq!(body["retryable"], false);
    assert_eq!(
        admission_counts(&rig.pool, component, submission).await,
        (0, 0, 0, 0)
    );
    let original = retain_historical_class50_fixture(&rig.pool, HISTORICAL_CLASS50_SUBJECT).await;
    let endpoint = format!("{uri}/{}", original.id);
    let (status, view) = send(&api, "GET", &endpoint, None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(view["subject_bytes"], HISTORICAL_CLASS50_SUBJECT);
    assert_eq!(view["receipt"]["candidate"]["class_code"], 50);
    assert_eq!(view["receipt"]["review_status"], "unreviewed");
    assert_eq!(view["receipt"]["catalogue_activated"], false);
    let candidate = PgComponentRevisionStore::new(rig.pool.clone())
        .read_revision(original.candidate.uuid, 1)
        .await
        .unwrap();
    let (status, recovered) = send(
        &api,
        "POST",
        uri,
        Some(&request(original.id, candidate.draft(), None, &[])),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(recovered["subject_checksum"], original.checksum);
    assert_eq!(
        admission_counts(&rig.pool, original.candidate.uuid, original.id).await,
        (1, 1, 1, 1)
    );
}

/// The mounted operator path must expose real ToolSkill rows and distinguish
/// an empty scoped inventory, an unknown ID, invalid input and a broken backend.
#[tokio::test]
async fn native_http_tool_skill_inventory_reads_real_scoped_rows_and_backend_errors() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let app = app(rig.pool.clone());
    let list_uri = "/api/webchat/v2/tool-skills?project_id=toolskill-project";
    let (status, body) = send(&app, "GET", list_uri, None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["tool_skills"], json!([]));
    assert_eq!(
        send(&app, "GET", list_uri, None, false).await.0,
        StatusCode::UNAUTHORIZED
    );
    let client = rig.pool.get().await.unwrap();
    let mut ids = Vec::new();
    for (tenant, user, agent, project, name, tool_name, state, content) in [
        (
            "submission-test-instance",
            "submission-test-operator",
            "default",
            "toolskill-project",
            "first",
            Some("ls"),
            "pending",
            "你好世界a",
        ),
        (
            "submission-test-instance",
            "submission-test-operator",
            "default",
            "toolskill-project",
            "second",
            None,
            "rejected",
            "",
        ),
        (
            "other-instance",
            "submission-test-operator",
            "default",
            "toolskill-project",
            "other-tenant",
            Some("ls"),
            "pending",
            "foreign",
        ),
        (
            "submission-test-instance",
            "other-operator",
            "default",
            "toolskill-project",
            "other-user",
            Some("ls"),
            "pending",
            "foreign",
        ),
        (
            "submission-test-instance",
            "submission-test-operator",
            "other-agent",
            "toolskill-project",
            "other-agent",
            Some("ls"),
            "pending",
            "foreign",
        ),
        (
            "submission-test-instance",
            "submission-test-operator",
            "default",
            "other-project",
            "other-project",
            Some("ls"),
            "pending",
            "foreign",
        ),
    ] {
        let id: Uuid = client.query_one(
            "INSERT INTO reborn_tool_skills
             (tenant_id,user_id,agent_id,project_id,name,description,content,tool_name,
              param_schema,param_template,validation_status,source,usage_count,success_count,
              failure_count,wilson_lower,consumer_tags)
             VALUES ($1,$2,$3,$4,$5,'real binding metadata',$6,$7,
                     '{\"type\":\"object\"}'::jsonb,'{\"all\":true}'::jsonb,$8,'authored',7,5,2,0.25,
                     ARRAY['00:rusty']) RETURNING id",
            &[&tenant,&user,&agent,&project,&name,&content,&tool_name,&state],
        ).await.unwrap().get(0);
        ids.push(id);
    }
    let (status, body) = send(&app, "GET", list_uri, None, true).await;
    assert_eq!(status, StatusCode::OK);
    let rows = body["tool_skills"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["id"], ids[0].to_string());
    assert_eq!(rows[0]["name"], "first");
    assert_eq!(rows[0]["tool_name"], "ls");
    assert_eq!(rows[0]["category"], "tool_skill");
    assert_eq!(rows[0]["estimated_tokens"], 2);
    assert_eq!(rows[0]["usage_count"], 7);
    assert_eq!(rows[0]["success_count"], 5);
    assert_eq!(rows[0]["failure_count"], 2);
    assert_eq!(rows[0]["wilson_lower"], 0.25);
    assert_eq!(rows[0]["validation_status"], "pending");
    assert!(rows[0]["created_at"].as_str().unwrap().contains('T'));
    assert_eq!(rows[1]["id"], ids[1].to_string());
    assert_eq!(rows[1]["tool_name"], "");
    assert_eq!(rows[1]["estimated_tokens"], 0);
    assert_eq!(rows[1]["validation_status"], "rejected");
    let detail_uri = format!("/api/webchat/v2/tool-skills/toolskill-project/{}", ids[0]);
    let (status, body) = send(&app, "GET", &detail_uri, None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], ids[0].to_string());
    let detail = &body["tool_skill"];
    assert_eq!(detail["content"], "你好世界a");
    assert_eq!(detail["class_code"], 13);
    assert_eq!(detail["param_schema"], json!({"type":"object"}));
    assert_eq!(detail["param_template"], json!({"all":true}));
    assert_eq!(detail["consumer_tags"], json!(["00:rusty"]));
    assert_eq!(detail["includes"], json!([]));
    assert!(detail["prior_knowledge_content"].is_null());
    assert!(detail.get("tenant_id").is_none());
    assert!(detail.get("code_snippet").is_none());
    for id in ids
        .iter()
        .skip(2)
        .copied()
        .chain(std::iter::once(Uuid::new_v4()))
    {
        let uri = format!("/api/webchat/v2/tool-skills/toolskill-project/{id}");
        assert_eq!(
            send(&app, "GET", &uri, None, true).await.0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        send(
            &app,
            "GET",
            "/api/webchat/v2/tool-skills/toolskill-project/not-a-uuid",
            None,
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    // Prior-knowledge override is the stored display/estimate source; it must
    // not be silently replaced by the raw content or become an execution cap.
    client
        .execute(
            "UPDATE reborn_tool_skills SET prior_knowledge_content=$2 WHERE id=$1",
            &[&ids[0], &"abcdefghi"],
        )
        .await
        .unwrap();
    let (status, body) = send(&app, "GET", list_uri, None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["tool_skills"][0]["estimated_tokens"], 3);
    let (status, body) = send(&app, "GET", &detail_uri, None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["tool_skill"]["content"], "你好世界a");
    assert_eq!(body["tool_skill"]["prior_knowledge_content"], "abcdefghi");
    let nullable_uri = format!("/api/webchat/v2/tool-skills/toolskill-project/{}", ids[1]);
    let (status, body) = send(&app, "GET", &nullable_uri, None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["tool_skill"]["tool_name"].is_null());
    // This native rig owns its database. A missing migration is not an empty
    // inventory; both existing HTTP read routes must report backend failure.
    client
        .batch_execute("ALTER TABLE reborn_tool_skills RENAME TO unavailable_tool_skills")
        .await
        .unwrap();
    assert_eq!(
        send(&app, "GET", list_uri, None, true).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        send(&app, "GET", &detail_uri, None, true).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    client
        .batch_execute("ALTER TABLE unavailable_tool_skills RENAME TO reborn_tool_skills")
        .await
        .unwrap();
    drop(client);
    rig.pool.close();
    assert_eq!(
        send(&app, "GET", list_uri, None, true).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        send(&app, "GET", &detail_uri, None, true).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
}

/// Sealed legacy queue state is not a substitute for the missing connected
/// ToolSkill functional-review writer. No existing action may acknowledge a no-op.
#[tokio::test]
async fn native_http_tool_skill_review_actions_fail_visibly_without_any_write() {
    use brassclaw_engine::memory::retrieval_source::ComponentScope;
    use brassclaw_product_workflow::{
        RebornServicesApi, UpdateValidationStatusRequest, WebUiAuthenticatedCaller,
    };

    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let app = app(rig.pool.clone());
    let scope = ComponentScope {
        tenant_id: "submission-test-instance".into(),
        user_id: "submission-test-operator".into(),
        agent_id: "default".into(),
        project_id: "toolskill-review".into(),
    };
    let queue = crate::validation_queue::ValidationQueueStore::new(rig.pool.clone());
    let client = rig.pool.get().await.unwrap();
    let id: Uuid = client
        .query_one(
            "INSERT INTO reborn_tool_skills
         (tenant_id,user_id,agent_id,project_id,name,description,content,tool_name,
          source,consumer_tags)
         VALUES ($1,$2,$3,$4,'review-target','actual ToolSkill','binding metadata','ls',
                 'authored',ARRAY['00:rusty','05:validator']) RETURNING id",
            &[
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
            ],
        )
        .await
        .unwrap()
        .get(0);
    let request = json!({"feedback":"operator review context"});
    for stage in 0..4 {
        match stage {
            1 => queue.submit(&scope, id, 13, None).await.unwrap(),
            // Queue lifecycle fixture, not execution of production Q1 or a
            // functional pass: even a sealed legacy queue cannot bypass this gap.
            2 => queue.gate1_pass(&scope, id, &[]).await.unwrap(),
            3 => {
                client.execute(
                    "UPDATE reborn_tool_skills SET content='changed since legacy Q1' WHERE id=$1",
                    &[&id],
                ).await.unwrap();
            }
            _ => {}
        }
        let before: String = client
            .query_one(
                "SELECT to_jsonb(c)::text FROM reborn_tool_skills c WHERE id=$1",
                &[&id],
            )
            .await
            .unwrap()
            .get(0);
        let queued: Option<String> = client
            .query_opt(
                "SELECT to_jsonb(q)::text FROM reborn_validation_queue q WHERE component_id=$1",
                &[&id],
            )
            .await
            .unwrap()
            .map(|row| row.get(0));
        for action in ["validate", "reject", "send-to-revision"] {
            let uri = format!(
                "/api/webchat/v2/components/13/{id}/{action}?project_id={}",
                scope.project_id,
            );
            let (status, body) = send(&app, "PUT", &uri, Some(&request), true).await;
            assert_eq!(
                status,
                StatusCode::SERVICE_UNAVAILABLE,
                "{action} stage {stage}: {body}"
            );
            assert!(body.get("new_status").is_none());
            let after: String = client
                .query_one(
                    "SELECT to_jsonb(c)::text FROM reborn_tool_skills c WHERE id=$1",
                    &[&id],
                )
                .await
                .unwrap()
                .get(0);
            assert_eq!(after, before);
            let after_queue: Option<String> = client
                .query_opt(
                    "SELECT to_jsonb(q)::text FROM reborn_validation_queue q WHERE component_id=$1",
                    &[&id],
                )
                .await
                .unwrap()
                .map(|row| row.get(0));
            assert_eq!(after_queue, queued);
            let receipts: i64 = client.query_one(
                "SELECT count(*) FROM reborn_component_graduation_receipts WHERE component_id=$1",
                &[&id],
            ).await.unwrap().get(0);
            assert_eq!(receipts, 0);
        }
    }
    // The three older service actions use the same facade method as the
    // generalized mounted routes; they must also return errors, not unknowns.
    let services = RebornServices::new(
        Arc::new(PgSessionThreadService::new(
            rig.pool.clone(),
            &scope.tenant_id,
        )),
        Arc::new(DefaultTurnCoordinator::new(Arc::new(
            PgTurnStateStore::new(rig.pool.clone(), &scope.tenant_id),
        ))),
    )
    .with_recipe_store(Arc::new(crate::pg_recipe_store::PgRecipeStoreFacade::new(
        rig.pool.clone(),
        &scope.tenant_id,
        &scope.agent_id,
    )));
    let caller = WebUiAuthenticatedCaller::new(
        TenantId::new(&scope.tenant_id).unwrap(),
        UserId::new(&scope.user_id).unwrap(),
        None,
        None,
    );
    let review_request = || UpdateValidationStatusRequest {
        feedback: Some("operator review context".into()),
    };
    assert!(
        services
            .validate_tool_skill(
                caller.clone(),
                &scope.project_id,
                &id.to_string(),
                review_request()
            )
            .await
            .is_err()
    );
    assert!(
        services
            .reject_tool_skill(
                caller.clone(),
                &scope.project_id,
                &id.to_string(),
                review_request()
            )
            .await
            .is_err()
    );
    assert!(
        services
            .request_tool_skill_review(caller, &scope.project_id, &id.to_string(), review_request())
            .await
            .is_err()
    );
    let uri = |value: &str, project: &str| {
        format!("/api/webchat/v2/components/13/{value}/validate?project_id={project}")
    };
    assert_eq!(
        send(
            &app,
            "PUT",
            &uri(&id.to_string(), &scope.project_id),
            Some(&request),
            false
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(
            &app,
            "PUT",
            &uri("not-a-uuid", &scope.project_id),
            Some(&request),
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(
            &app,
            "PUT",
            &uri(&Uuid::new_v4().to_string(), &scope.project_id),
            Some(&request),
            true
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(
            &app,
            "PUT",
            &uri(&id.to_string(), "other-project"),
            Some(&request),
            true
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    client
        .batch_execute("ALTER TABLE reborn_tool_skills RENAME TO unavailable_tool_skills")
        .await
        .unwrap();
    assert_eq!(
        send(
            &app,
            "PUT",
            &uri(&id.to_string(), &scope.project_id),
            Some(&request),
            true
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    client
        .batch_execute("ALTER TABLE unavailable_tool_skills RENAME TO reborn_tool_skills")
        .await
        .unwrap();
}

/// Real legacy candidate/queue fixtures for atomic rejection tests. These seals
/// exercise custody, not production functional Q1 or immutable own approval.
async fn recipe_rejection_candidate(
    pool: &Arc<PgPool>,
    name: &str,
    upgrade: bool,
) -> (
    Uuid,
    brassclaw_engine::memory::retrieval_source::ComponentScope,
) {
    use brassclaw_engine::memory::retrieval_source::ComponentScope;
    let scope = ComponentScope {
        tenant_id: "submission-test-instance".into(),
        user_id: "submission-test-operator".into(),
        agent_id: "default".into(),
        project_id: "recipe-rejection".into(),
    };
    let status = if upgrade { "validated" } else { "pending" };
    let id = pool
        .get()
        .await
        .unwrap()
        .query_one(
            "INSERT INTO reborn_recipes
         (tenant_id,user_id,agent_id,project_id,name,description,validation_status)
         VALUES ($1,$2,$3,$4,$5,'original Recipe',$6) RETURNING id",
            &[
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &name,
                &status,
            ],
        )
        .await
        .unwrap()
        .get(0);
    (id, scope)
}

async fn recipe_rejection_snapshot(pool: &PgPool, id: Uuid) -> (String, Option<String>) {
    let client = pool.get().await.unwrap();
    let recipe = client
        .query_one(
            "SELECT to_jsonb(c)::text FROM reborn_recipes c WHERE id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    let queue = client
        .query_opt(
            "SELECT to_jsonb(q)::text FROM reborn_validation_queue q WHERE component_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .map(|row| row.get(0));
    (recipe, queue)
}

#[tokio::test]
async fn native_http_recipe_manual_rejection_preserves_non_q2_queue_and_upgrade_live_row() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let app = app(rig.pool.clone());
    let queue = crate::validation_queue::ValidationQueueStore::new(rig.pool.clone());
    let request = json!({"feedback":"needs revision"});
    for name in ["no-queue", "pending-queue", "q2-queue"] {
        let (id, scope) = recipe_rejection_candidate(&rig.pool, name, false).await;
        if name != "no-queue" {
            queue.submit(&scope, id, 21, None).await.unwrap();
        }
        if name == "q2-queue" {
            queue.gate1_pass(&scope, id, &[]).await.unwrap();
        }
        let before = recipe_rejection_snapshot(&rig.pool, id).await;
        let uri = format!(
            "/api/webchat/v2/components/21/{id}/reject?project_id={}",
            scope.project_id
        );
        let (status, body) = send(&app, "PUT", &uri, Some(&request), true).await;
        assert_eq!(status, StatusCode::OK, "{name}: {body}");
        assert_eq!(body["previous_status"], "pending");
        assert_eq!(body["new_status"], "rejected");
        let after = recipe_rejection_snapshot(&rig.pool, id).await;
        let recipe: Value = serde_json::from_str(&after.0).unwrap();
        assert_eq!(recipe["validation_status"], "rejected");
        if name == "q2-queue" {
            let rejected: Value = serde_json::from_str(after.1.as_ref().unwrap()).unwrap();
            assert_eq!(rejected["state"], 3);
            assert_eq!(rejected["counter"], 1);
            assert_eq!(rejected["review_feedback"], "needs revision");
        } else {
            assert_eq!(
                after.1, before.1,
                "manual metadata action must not manufacture Q2"
            );
        }
        // Repeated manual metadata rejection is idempotent; an existing state3
        // ticket gets no extra Q2 rejection/counter/feedback write.
        assert_eq!(
            send(&app, "PUT", &uri, Some(&request), true).await.0,
            StatusCode::OK
        );
        assert_eq!(recipe_rejection_snapshot(&rig.pool, id).await, after);
        let receipts: i64 = rig
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM reborn_component_graduation_receipts WHERE component_id=$1",
                &[&id],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(receipts, 0);
    }
    let (id, scope) = recipe_rejection_candidate(&rig.pool, "upgrade-recipe", true).await;
    queue
        .submit(
            &scope,
            id,
            21,
            Some(json!({"description":"proposed upgrade"})),
        )
        .await
        .unwrap();
    let uri = format!(
        "/api/webchat/v2/components/21/{id}/reject?project_id={}",
        scope.project_id
    );
    let before = recipe_rejection_snapshot(&rig.pool, id).await;
    // No premature Q2 or invented cancellation of a pending upgrade proposal.
    assert_eq!(
        send(&app, "PUT", &uri, Some(&request), true).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(recipe_rejection_snapshot(&rig.pool, id).await, before);
    queue.gate1_pass(&scope, id, &[]).await.unwrap();
    let before = recipe_rejection_snapshot(&rig.pool, id).await;
    let client = rig.pool.get().await.unwrap();
    client
        .execute(
            "UPDATE reborn_validation_queue SET component_class=13 WHERE component_id=$1",
            &[&id],
        )
        .await
        .unwrap();
    let wrong_class = recipe_rejection_snapshot(&rig.pool, id).await;
    assert_eq!(
        send(&app, "PUT", &uri, Some(&request), true).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(recipe_rejection_snapshot(&rig.pool, id).await, wrong_class);
    client
        .execute(
            "UPDATE reborn_validation_queue SET component_class=21 WHERE component_id=$1",
            &[&id],
        )
        .await
        .unwrap();
    let (status, body) = send(&app, "PUT", &uri, Some(&request), true).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["previous_status"], "validated");
    assert_eq!(body["new_status"], "validated");
    let after = recipe_rejection_snapshot(&rig.pool, id).await;
    assert_eq!(
        after.0, before.0,
        "live upgrade bytes must be preserved exactly"
    );
    let rejected: Value = serde_json::from_str(after.1.as_ref().unwrap()).unwrap();
    assert_eq!(rejected["state"], 3);
    assert_eq!(rejected["counter"], 1);
    assert_eq!(rejected["review_feedback"], "needs revision");
    assert_eq!(
        rejected["proposed_payload"],
        json!({"description":"proposed upgrade"})
    );
    assert_eq!(
        send(&app, "PUT", &uri, Some(&request), true).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(recipe_rejection_snapshot(&rig.pool, id).await, after);
}

#[tokio::test]
async fn native_http_recipe_rejection_rolls_back_late_statement_and_commit_failures() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let app = app(rig.pool.clone());
    let queue = crate::validation_queue::ValidationQueueStore::new(rig.pool.clone());
    let client = rig.pool.get().await.unwrap();
    client
        .batch_execute(
            "CREATE FUNCTION fail_recipe_reject() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'private rejection diagnostic' USING ERRCODE='23514'; END $$;",
        )
        .await
        .unwrap();
    for mode in ["no-queue", "pending-queue", "q2-queue", "upgrade-queue"] {
        let upgrade = mode == "upgrade-queue";
        let (id, scope) =
            recipe_rejection_candidate(&rig.pool, &format!("failure-{mode}"), upgrade).await;
        if mode != "no-queue" {
            queue
                .submit(
                    &scope,
                    id,
                    21,
                    upgrade.then(|| json!({"description":"proposed upgrade"})),
                )
                .await
                .unwrap();
        }
        if mode == "q2-queue" || upgrade {
            queue.gate1_pass(&scope, id, &[]).await.unwrap();
        }
        let before = recipe_rejection_snapshot(&rig.pool, id).await;
        let uri = format!(
            "/api/webchat/v2/components/21/{id}/reject?project_id={}",
            scope.project_id
        );
        let request = json!({"feedback":"rejected atomically"});
        // A real queue-table failure is never treated like expected absence or
        // non-Q2 state, including the otherwise supported manual row action.
        client
            .batch_execute(
                "ALTER TABLE reborn_validation_queue RENAME TO unavailable_rejection_queue",
            )
            .await
            .unwrap();
        assert_eq!(
            send(&app, "PUT", &uri, Some(&request), true).await.0,
            StatusCode::SERVICE_UNAVAILABLE
        );
        client
            .batch_execute(
                "ALTER TABLE unavailable_rejection_queue RENAME TO reborn_validation_queue",
            )
            .await
            .unwrap();
        assert_eq!(recipe_rejection_snapshot(&rig.pool, id).await, before);
        let (table, condition) = if upgrade {
            ("reborn_validation_queue", "NEW.state IN (3,4)")
        } else {
            ("reborn_recipes", "NEW.validation_status='rejected'")
        };
        for deferred in [false, true] {
            let ddl = if deferred {
                format!(
                    "CREATE CONSTRAINT TRIGGER recipe_reject_fault AFTER UPDATE ON {table} DEFERRABLE INITIALLY DEFERRED FOR EACH ROW WHEN ({condition}) EXECUTE FUNCTION fail_recipe_reject()"
                )
            } else {
                format!(
                    "CREATE TRIGGER recipe_reject_fault BEFORE UPDATE ON {table} FOR EACH ROW WHEN ({condition}) EXECUTE FUNCTION fail_recipe_reject()"
                )
            };
            client.batch_execute(&ddl).await.unwrap();
            let (status, body) = send(&app, "PUT", &uri, Some(&request), true).await;
            assert_eq!(
                status,
                StatusCode::SERVICE_UNAVAILABLE,
                "{mode}, deferred={deferred}: {body}"
            );
            assert!(!body.to_string().contains("private rejection diagnostic"));
            assert_eq!(recipe_rejection_snapshot(&rig.pool, id).await, before);
            client
                .batch_execute(&format!("DROP TRIGGER recipe_reject_fault ON {table}"))
                .await
                .unwrap();
        }
        // Explicit retry after removing the fault commits only supported work.
        assert_eq!(
            send(&app, "PUT", &uri, Some(&request), true).await.0,
            StatusCode::OK
        );
        let after = recipe_rejection_snapshot(&rig.pool, id).await;
        let recipe: Value = serde_json::from_str(&after.0).unwrap();
        if upgrade {
            assert_eq!(after.0, before.0);
            assert_eq!(recipe["validation_status"], "validated");
        } else {
            assert_eq!(recipe["validation_status"], "rejected");
        }
        if mode == "q2-queue" || upgrade {
            let rejected: Value = serde_json::from_str(after.1.as_ref().unwrap()).unwrap();
            assert_eq!(rejected["state"], 3);
            assert_eq!(rejected["counter"], 1);
            assert_eq!(rejected["review_feedback"], "rejected atomically");
        } else {
            assert_eq!(
                after.1, before.1,
                "manual action must leave non-Q2 queue intact"
            );
        }
    }
}

/// Observe the contender waiting on this transaction's exact advisory key.
/// Completion is driven by actual PostgreSQL lock state, never a sleep race.
async fn observe_queue_key_waiter(holder: &tokio_postgres::Transaction<'_>) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let waiting: bool = holder
                .query_one(
                    "SELECT EXISTS (
                    SELECT 1 FROM pg_locks waiting JOIN pg_locks mine
                    ON waiting.database=mine.database AND waiting.classid=mine.classid
                       AND waiting.objid=mine.objid AND waiting.objsubid=mine.objsubid
                    WHERE mine.locktype='advisory' AND waiting.locktype='advisory'
                      AND mine.pid=pg_backend_pid() AND mine.granted
                      AND NOT waiting.granted AND waiting.pid<>mine.pid)",
                    &[],
                )
                .await
                .unwrap()
                .get(0);
            if waiting {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actual competing owner waits for this transaction's exact queue key");
}

#[tokio::test]
async fn native_pg_queue_absence_and_upgrade_insert_share_transaction_key() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let app = app(rig.pool.clone());
    let queue = crate::validation_queue::ValidationQueueStore::new(rig.pool.clone());
    let (id, scope) = recipe_rejection_candidate(&rig.pool, "held-absence", true).await;
    let mut holder_client = rig.pool.get().await.unwrap();
    let holder = holder_client.transaction().await.unwrap();
    assert!(
        queue
            .lock_for_rejection(&holder, &scope, id)
            .await
            .unwrap()
            .is_none()
    );
    // First ordering: an actual empty queue read holds its key until transaction
    // end. The public submit operation must not autocommit its lock prematurely.
    let (submitted, ()) = tokio::join!(
        queue.submit(
            &scope,
            id,
            21,
            Some(json!({"description":"queued after reader"}))
        ),
        async {
            observe_queue_key_waiter(&holder).await;
            let visible: i64 = holder
                .query_one(
                    "SELECT count(*) FROM reborn_validation_queue WHERE component_id=$1",
                    &[&id],
                )
                .await
                .unwrap()
                .get(0);
            assert_eq!(visible, 0);
            holder.commit().await.unwrap();
        },
    );
    submitted.unwrap();
    let inserted = recipe_rejection_snapshot(&rig.pool, id).await;
    let row: Value = serde_json::from_str(inserted.1.as_ref().unwrap()).unwrap();
    assert_eq!(
        row["proposed_payload"],
        json!({"description":"queued after reader"})
    );
    // Second ordering: the actual transaction-taking submission holds its key
    // before commit. The mounted rejection blocks, then sees that exact proposal
    // instead of caching a false absence and marking the live Recipe rejected.
    let (id, scope) = recipe_rejection_candidate(&rig.pool, "held-upgrade", true).await;
    let original = recipe_rejection_snapshot(&rig.pool, id).await;
    let holder = holder_client.transaction().await.unwrap();
    queue
        .submit_in_transaction(
            &holder,
            &scope,
            id,
            21,
            Some(json!({"description":"exact concurrent proposal"})),
        )
        .await
        .unwrap();
    let uri = format!(
        "/api/webchat/v2/components/21/{id}/reject?project_id={}",
        scope.project_id
    );
    let request = json!({"feedback":"concurrent manual rejection"});
    let ((status, body), ()) = tokio::join!(send(&app, "PUT", &uri, Some(&request), true), async {
        observe_queue_key_waiter(&holder).await;
        holder.commit().await.unwrap();
    },);
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "pending upgrade uses existing strict Q2 contract: {body}"
    );
    let after = recipe_rejection_snapshot(&rig.pool, id).await;
    assert_eq!(
        after.0, original.0,
        "no live mutation after stale empty-queue read"
    );
    let proposal: Value = serde_json::from_str(after.1.as_ref().unwrap()).unwrap();
    assert_eq!(proposal["state"], 1);
    assert_eq!(proposal["counter"], 0);
    assert!(proposal["review_feedback"].is_null());
    assert_eq!(
        proposal["proposed_payload"],
        json!({"description":"exact concurrent proposal"})
    );
}

/// Actual legacy metadata fixtures, not successful Q1/behavior/approval evidence.
async fn re_review_metadata_candidate(
    pool: &Arc<PgPool>,
    name: &str,
    with_queue: bool,
) -> (
    Uuid,
    brassclaw_engine::memory::retrieval_source::ComponentScope,
) {
    let (id, scope) = recipe_rejection_candidate(pool, name, false).await;
    pool.get()
        .await
        .unwrap()
        .execute(
            "UPDATE reborn_recipes SET validation_status='rejected',
             consumer_tags=ARRAY['02:orchestrator','05:validator','00:rusty','05:validator']
         WHERE id=$1",
            &[&id],
        )
        .await
        .unwrap();
    if with_queue {
        // Use the actual queue owner and its configured threshold, then a
        // lifecycle-only seal. The action must preserve this existing Q4 row.
        let queue =
            crate::validation_queue::ValidationQueueStore::with_reject_threshold(pool.clone(), 1);
        queue.submit(&scope, id, 21, None).await.unwrap();
        queue.gate1_pass(&scope, id, &[]).await.unwrap();
        queue
            .reject(&scope, id, "existing operator rejection")
            .await
            .unwrap();
    }
    (id, scope)
}

#[tokio::test]
async fn native_http_recipe_re_review_commits_status_tag_and_preserves_queue_metadata() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let app = app(rig.pool.clone());
    let request = json!({"feedback":"operator requests another review"});
    for (name, with_queue) in [("manual-rereview", false), ("q4-rereview", true)] {
        let (id, scope) = re_review_metadata_candidate(&rig.pool, name, with_queue).await;
        let before = recipe_rejection_snapshot(&rig.pool, id).await;
        let uri = format!(
            "/api/webchat/v2/components/21/{id}/re-review?project_id={}",
            scope.project_id
        );
        let (status, body) = send(&app, "PUT", &uri, Some(&request), true).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["previous_status"], "rejected");
        assert_eq!(body["new_status"], "pending");
        let after = recipe_rejection_snapshot(&rig.pool, id).await;
        let mut old_row: Value = serde_json::from_str(&before.0).unwrap();
        let mut new_row: Value = serde_json::from_str(&after.0).unwrap();
        assert_eq!(new_row["validation_status"], "pending");
        assert_eq!(
            new_row["consumer_tags"],
            json!(["02:orchestrator", "00:rusty", "05:validator"])
        );
        for field in ["validation_status", "consumer_tags", "updated_at"] {
            old_row.as_object_mut().unwrap().remove(field);
            new_row.as_object_mut().unwrap().remove(field);
        }
        assert_eq!(
            new_row, old_row,
            "preserve all unrelated actual metadata bytes"
        );
        assert_eq!(after.1, before.1, "manual action creates no queue verdict");
        assert_eq!(
            send(&app, "PUT", &uri, Some(&request), true).await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(recipe_rejection_snapshot(&rig.pool, id).await, after);
        let outside =
            format!("/api/webchat/v2/components/21/{id}/re-review?project_id=other-project");
        assert_eq!(
            send(&app, "PUT", &outside, Some(&request), true).await.0,
            StatusCode::NOT_FOUND
        );
        let evidence: i64 = rig.pool.get().await.unwrap().query_one(
            "SELECT (SELECT count(*) FROM reborn_component_graduation_receipts WHERE component_id=$1)
                 + (SELECT count(*) FROM reborn_component_review_evidence)", &[&id],
        ).await.unwrap().get(0);
        assert_eq!(evidence, 0, "metadata is not approval evidence");
    }
}

#[tokio::test]
async fn native_http_recipe_re_review_rolls_back_tag_statement_and_commit_failures() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let app = app(rig.pool.clone());
    let client = rig.pool.get().await.unwrap();
    client
        .batch_execute(
            "CREATE FUNCTION fail_rereview_tag() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'private rereview diagnostic' USING ERRCODE='23514'; END $$;",
        )
        .await
        .unwrap();
    for (name, with_queue) in [
        ("manual-rereview-fault", false),
        ("q4-rereview-fault", true),
    ] {
        let (id, scope) = re_review_metadata_candidate(&rig.pool, name, with_queue).await;
        let before = recipe_rejection_snapshot(&rig.pool, id).await;
        let uri = format!(
            "/api/webchat/v2/components/21/{id}/re-review?project_id={}",
            scope.project_id
        );
        let request = json!({"feedback":"retry metadata only"});
        for ddl in [
            // First status UPDATE has succeeded before the actual tag statement fails.
            "CREATE TRIGGER rereview_fault BEFORE UPDATE OF consumer_tags ON reborn_recipes FOR EACH ROW EXECUTE FUNCTION fail_rereview_tag()",
            // Both actual statements succeed; the real commit is then rejected.
            "CREATE CONSTRAINT TRIGGER rereview_fault AFTER UPDATE ON reborn_recipes DEFERRABLE INITIALLY DEFERRED FOR EACH ROW WHEN (OLD.consumer_tags IS DISTINCT FROM NEW.consumer_tags) EXECUTE FUNCTION fail_rereview_tag()",
        ] {
            client.batch_execute(ddl).await.unwrap();
            let (status, body) = send(&app, "PUT", &uri, Some(&request), true).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
            assert!(!body.to_string().contains("private rereview diagnostic"));
            assert_eq!(recipe_rejection_snapshot(&rig.pool, id).await, before);
            client
                .batch_execute("DROP TRIGGER rereview_fault ON reborn_recipes")
                .await
                .unwrap();
        }
        assert_eq!(
            send(&app, "PUT", &uri, Some(&request), true).await.0,
            StatusCode::OK
        );
        let after = recipe_rejection_snapshot(&rig.pool, id).await;
        let row: Value = serde_json::from_str(&after.0).unwrap();
        assert_eq!(row["validation_status"], "pending");
        assert_eq!(
            row["consumer_tags"],
            json!(["02:orchestrator", "00:rusty", "05:validator"])
        );
        assert_eq!(after.1, before.1);
    }
}
