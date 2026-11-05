//! Real PG/operator capture and lifecycle tests. Direct gate1 calls below are
//! lifecycle fixtures, not structural/behavioral Q1 or component approval proof.
use super::*;
use crate::validation_queue::{ValidationQueueError, ValidationQueueStore};
use brassclaw_engine::memory::retrieval_source::ComponentScope;
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft,
    review_submission_store::{ReviewSubmissionDraft, queue},
};
use serde_json::{Value, json};
use tokio_postgres::IsolationLevel;

fn document(name: &str) -> Value {
    json!({"name":name,"description":"Original complete queue capture","content":"def retain(inputs):\n    return inputs['text']\n",
        "consumer_tags":["02:orchestrator"],"prior_knowledge_content":null,"override_prompt_creation":false,
        "intent_examples":null,"dependency_registry":null,"includes":[],
        "similarity_parent_id":null,"replaces_id":null,"parent_version":null,
        "input_contract":{"text":{"type":"string","required":true,"checks":[]}},"result_contract":{"type":"string"},
        "preload":{"format":"python-preload/2","exports":{"retain":{"symbol":"retain","parameters":["inputs"],"mapping":true}},
        "private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"retain"}})
}
fn proposal(operation: Uuid, component: Uuid, document: Value) -> Value {
    json!({"submission_id":operation,"candidate_bytes":json!({"format":"component-revision/1","uuid":component,"class_code":22,
        "document":document,"dependencies":[],"association":null}).to_string(),"base":null,"dependencies":[],"create_queue_project":"default"})
}
fn scope() -> ComponentScope {
    ComponentScope {
        tenant_id: "submission-test-instance".into(),
        user_id: "submission-test-operator".into(),
        agent_id: "default".into(),
        project_id: "default".into(),
    }
}
const URI: &str = "/api/webchat/v2/component-review-submissions";

#[tokio::test]
async fn original_queue_operator_create_retry_and_q2_historical_readback() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let app = super::tests::app(rig.pool.clone());
    let component = Uuid::new_v4();
    let operation = Uuid::new_v4();
    let request = proposal(operation, component, document("original-queued-code"));
    let (status, receipt) = super::tests::send(&app, "POST", URI, Some(&request), true).await;
    assert!(status.is_success(), "{status}: {receipt}");
    let queue_store = ValidationQueueStore::new(rig.pool.clone());
    let captured = queue_store
        .capture_q1_candidate(&scope(), component)
        .await
        .unwrap();
    let queue_id = captured.original().unwrap().queue_id();
    assert_eq!(captured.original().unwrap().submission_id(), operation);
    let (_, retry) = super::tests::send(&app, "POST", URI, Some(&request), true).await;
    assert_eq!(receipt, retry);
    let mut changed = request.clone();
    let mut revision: Value =
        serde_json::from_str(changed["candidate_bytes"].as_str().unwrap()).unwrap();
    revision["document"]["preload"]["default_export"] = json!("other");
    changed["candidate_bytes"] = json!(revision.to_string());
    let (status, _) = super::tests::send(&app, "POST", URI, Some(&changed), true).await;
    assert!(!status.is_success());
    queue_store
        .gate1_pass_reviewed(&scope(), &captured, &[])
        .await
        .unwrap();
    queue_store
        .approve(&scope(), component, Some("human"))
        .await
        .unwrap();
    assert!(queue_store.list(&scope(), None).await.unwrap().is_empty());
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let historical = queue::read_in_transaction(&tx, queue_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(historical.submission_id(), operation);
    tx.commit().await.unwrap();
    drop(client);
    let (status, retry) = super::tests::send(&app, "POST", URI, Some(&request), true).await;
    assert!(status.is_success());
    assert_eq!(receipt, retry);
    assert!(
        queue_store.list(&scope(), None).await.unwrap().is_empty(),
        "original readback must not create another queue"
    );
}

#[tokio::test]
async fn original_queue_a_cannot_certify_b_or_changed_cas() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let app = super::tests::app(rig.pool.clone());
    let component = Uuid::new_v4();
    let operation = Uuid::new_v4();
    let request = proposal(operation, component, document("queue-subject-a"));
    let (status, _) = super::tests::send(&app, "POST", URI, Some(&request), true).await;
    assert!(status.is_success());
    let store = ValidationQueueStore::new(rig.pool.clone());
    let review = store
        .capture_q1_candidate(&scope(), component)
        .await
        .unwrap();
    // A later standalone B retains its own exact subject but cannot replace A's
    // original queue binding or be mistaken for the selected Q1 subject.
    let mut b = document("queue-subject-a");
    b["content"] = json!("def other():\n    return 2\n");
    let draft=ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":component,"class_code":22,"document":b,"dependencies":[],"association":null}).to_string()).unwrap();
    let base = review.original().unwrap().candidate().unwrap();
    let later =
        ReviewSubmissionDraft::new(Uuid::new_v4(), "operator", draft, Some(base), vec![]).unwrap();
    let retained =
        brassclaw_skills::review_submission_store::PgReviewSubmissionStore::new(rig.pool.clone())
            .submit(&later)
            .await
            .unwrap();
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let wrong=brassclaw_skills::review_submission_store::PgReviewSubmissionStore::read_validator_subject_in_transaction(&tx,retained.id,brassclaw_skills::component_revision::REVISION_LIMITS).await.unwrap().unwrap();
    assert!(review.original().unwrap().require_subject(&wrong).is_err());
    tx.commit().await.unwrap();
    drop(client);
    assert_eq!(
        store
            .capture_q1_candidate(&scope(), component)
            .await
            .unwrap()
            .original()
            .unwrap()
            .submission_id(),
        operation
    );
    rig.pool.get().await.unwrap().execute("UPDATE reborn_python_code SET prior_knowledge_content='changed outside body' WHERE id=$1",&[&component]).await.unwrap();
    assert!(matches!(
        store.gate1_pass_reviewed(&scope(), &review, &[]).await,
        Err(ValidationQueueError::ReviewChanged { .. })
    ));
    assert!(matches!(
        store
            .gate1_fail_reviewed(&scope(), &review, &["actual negative".into()])
            .await,
        Err(ValidationQueueError::ReviewChanged { .. })
    ));
    assert_eq!(store.list(&scope(), None).await.unwrap()[0].state, 1);
}

#[tokio::test]
async fn original_queue_competing_authors_keep_one_atomic_original() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let component = Uuid::new_v4();
    let first = proposal(Uuid::new_v4(), component, document("concurrent-original"));
    let second = proposal(Uuid::new_v4(), component, document("concurrent-original"));
    let a = super::tests::app(rig.pool.clone());
    let b = a.clone();
    let (a, b) = tokio::join!(
        super::tests::send(&a, "POST", URI, Some(&first), true),
        super::tests::send(&b, "POST", URI, Some(&second), true)
    );
    assert_ne!(a.0.is_success(), b.0.is_success());
    let client = rig.pool.get().await.unwrap();
    let row=client.query_one("SELECT (SELECT count(*) FROM reborn_component_review_submissions WHERE component_id=$1),(SELECT count(*) FROM reborn_component_review_queue_bindings WHERE component_id=$1),(SELECT count(*) FROM reborn_component_revisions WHERE component_id=$1)",&[&component]).await.unwrap();
    assert_eq!(
        (
            row.get::<_, i64>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2)
        ),
        (1, 1, 1)
    );
}

#[tokio::test]
async fn original_queue_lost_commit_ack_reads_the_original_without_reinsert() {
    use brassclaw_pg::test_support::{AckCutRule, AckCutTransport};
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let transport =
        AckCutTransport::start(rig.url.expose_secret(), vec![AckCutRule::next_commit(1)])
            .await
            .unwrap();
    let app = super::tests::app(transport.pool.clone());
    let component = Uuid::new_v4();
    let request = proposal(Uuid::new_v4(), component, document("ack-original"));
    transport.arm(1).unwrap();
    let (status, _) = super::tests::send(&app, "POST", URI, Some(&request), true).await;
    assert_eq!(
        status,
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "lost commit ACK must remain infrastructure uncertainty"
    );
    assert_eq!(transport.cut_count(), 1);
    let native = super::tests::app(rig.pool.clone());
    let (status, original) = super::tests::send(&native, "POST", URI, Some(&request), true).await;
    assert!(status.is_success(), "{status}: {original}");
    let count: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM reborn_component_review_queue_bindings WHERE component_id=$1",
            &[&component],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 1);
    transport.shutdown().await.unwrap();
}

#[tokio::test]
async fn original_queue_joint_context_is_retained_and_never_partially_graduated() {
    use brassclaw_skills::{
        review_submission_store::FunctionReviewPartnerDraft,
        revision_store::PgComponentRevisionStore,
    };
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let code = Uuid::new_v4();
    let skill = Uuid::new_v4();
    let tool = Uuid::new_v4();
    let binding = Uuid::new_v4();
    let revisions = PgComponentRevisionStore::new(rig.pool.clone());
    let primitive = |id, class, dependencies| {
        ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":class,
        "document":{"name":"unreviewed context prerequisite"},"dependencies":dependencies,"association":null}).to_string()).unwrap()
    };
    let tool_ref = revisions
        .stage(&primitive(tool, 0, vec![]), 0)
        .await
        .unwrap();
    let binding_ref = revisions
        .stage(&primitive(binding, 13, vec![tool]), 0)
        .await
        .unwrap();
    let association = json!({"format":"skill-association/1","skill_uuid":skill,"python_code_uuid":code,"tool_skill_uuid":binding,"tool_uuid":tool,
        "callable":"host.read_file","inputs":{},"arguments":{},"code_arguments":{},"result":{"type":"string"},
        "failure":{"action":"stop","max_attempts":1,"idempotency":"not_assumed","idempotency_evidence_ref":null,"retryable_outcomes":[]}});
    let context=ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":skill,"class_code":1,"document":{"body":"exact unchanged partner"},
        "dependencies":[code,binding,tool],"association":association.to_string()}).to_string()).unwrap();
    let context_ref = revisions.stage(&context, 0).await.unwrap();
    let doc = document("joint-code");
    let sc = scope();
    rig.pool.get().await.unwrap().execute("INSERT INTO reborn_python_code(id,tenant_id,user_id,agent_id,project_id,name,description,content,consumer_tags) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        &[&code,&sc.tenant_id,&sc.user_id,&sc.agent_id,&sc.project_id,&"joint-code",&"Original complete queue capture",&doc["content"].as_str().unwrap(),&vec!["02:orchestrator".to_string()]]).await.unwrap();
    let store = ValidationQueueStore::new(rig.pool.clone());
    store.submit(&sc, code, 22, None).await.unwrap();
    let queue_id = store.list(&sc, None).await.unwrap()[0].id;
    let candidate=ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":code,"class_code":22,"document":doc,"dependencies":[],"association":null}).to_string()).unwrap();
    let original = ReviewSubmissionDraft::new_function(
        Uuid::new_v4(),
        "operator",
        candidate,
        None,
        vec![tool_ref, binding_ref],
        0,
        FunctionReviewPartnerDraft::Context {
            reference: context_ref,
        },
    )
    .unwrap();
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let subject = ValidationQueueStore::submit_original_in_transaction(
        &tx,
        &original,
        &[(sc.clone(), code, queue_id)],
        &[queue_id],
        "existing_queue",
    )
    .await
    .unwrap();
    assert!(
        subject
            .function_partner
            .is_some_and(|p| !p.changed && p.reference == context_ref)
    );
    tx.commit().await.unwrap();
    drop(client);
    let captured = store.capture_q1_candidate(&sc, code).await.unwrap();
    assert!(matches!(
        store.gate1_pass_reviewed(&sc, &captured, &[]).await,
        Err(ValidationQueueError::OriginalReviewUnresolved { .. })
    ));
    assert!(matches!(
        store
            .gate1_fail_reviewed(&sc, &captured, &["partial class check".into()])
            .await,
        Err(ValidationQueueError::OriginalReviewUnresolved { .. })
    ));
    assert_eq!(store.list(&sc, None).await.unwrap()[0].state, 1);
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let saved = queue::read_in_transaction(&tx, queue_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.submission_id(), subject.id);
    let reread = PgReviewSubmissionStore::read_in_transaction(&tx, subject.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reread.function_partner.unwrap().reference, context_ref);
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn original_queue_recipe_and_catalogue_create_preserve_complete_documents() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let app = super::tests::app(rig.pool.clone());
    for class in [21, 23] {
        let component = Uuid::new_v4();
        let operation = Uuid::new_v4();
        let mut doc = json!({"name":format!("queued-projection-{class}"),"description":"Complete original projection fixture",
            "consumer_tags":["02:orchestrator"],"prior_knowledge_content":null,"override_prompt_creation":false,
            "intent_examples":null,"dependency_registry":null,
            "similarity_parent_id":null,"replaces_id":null,"parent_version":null});
        if class == 21 {
            doc["status"] = json!("active");
            doc["trigger"] = Value::Null;
            doc["steps"] = json!([]);
            doc["step_descriptions"] = json!([]);
            doc["variants"] = json!([]);
            doc["validates_class_code"] = Value::Null;
            doc["input_layouts"] = json!({});
            doc["invocations"] = json!({});
        } else {
            doc["version"] = json!("1.0.0");
            doc["overview_doc"] = json!("Original exact overview");
            doc["task_groups"] = json!([]);
            doc["child_component_ids"] = json!([]);
            doc["intent_index"] = Value::Null;
        }
        // These are unreviewed drafts, deliberately not runnable/approved test
        // workflows. Capture must preserve even invalid drafts for real review.
        let candidate=json!({"format":"component-revision/1","uuid":component,"class_code":class,"document":doc,"dependencies":[],"association":null}).to_string();
        let input = json!({"submission_id":operation,"candidate_bytes":candidate,"base":null,"dependencies":[],"create_queue_project":"default"});
        let (status, receipt) = super::tests::send(&app, "POST", URI, Some(&input), true).await;
        assert!(status.is_success(), "class {class}: {status}: {receipt}");
        assert_eq!(receipt["queue_ids"].as_array().unwrap().len(), 1);
        let subject = PgReviewSubmissionStore::new(rig.pool.clone())
            .read(operation)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            subject.snapshot.revisions()[&component]
                .draft()
                .exact_bytes(),
            candidate
        );
    }
}

#[tokio::test]
async fn original_queue_foreign_payload_and_historical_rows_fail_without_subject_allocation() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let app = super::tests::app(rig.pool.clone());
    let component = Uuid::new_v4();
    let initial = proposal(Uuid::new_v4(), component, document("original-draft"));
    let (status, receipt) = super::tests::send(&app, "POST", URI, Some(&initial), true).await;
    assert!(status.is_success());
    let queue_id: Uuid = receipt["queue_ids"][0].as_str().unwrap().parse().unwrap();
    let mut foreign = initial.clone();
    foreign["submission_id"] = json!(Uuid::new_v4());
    foreign
        .as_object_mut()
        .unwrap()
        .remove("create_queue_project");
    foreign["queue_ids"] = json!([queue_id]);
    // Even identical bytes under a new original cannot take over this queue.
    foreign["expected_previous_version"] = json!(1);
    foreign["base"] = receipt["candidate"].clone();
    let (status, _) = super::tests::send(&app, "POST", URI, Some(&foreign), true).await;
    assert!(!status.is_success());
    let count: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM reborn_component_review_submissions WHERE component_id=$1",
            &[&component],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 1);
    // Migration marks only new queue generations eligible. An old unbound
    // pending row cannot be heuristically attached, even by a valid new subject.
    let other = Uuid::new_v4();
    let sc = scope();
    let client = rig.pool.get().await.unwrap();
    client.execute("INSERT INTO reborn_python_code(id,tenant_id,user_id,agent_id,project_id,name,description,content,consumer_tags) VALUES($1,$2,$3,$4,$5,'historical-code','Original complete queue capture',$6,$7)",
        &[&other,&sc.tenant_id,&sc.user_id,&sc.agent_id,&sc.project_id,&document("historical-code")["content"].as_str().unwrap(),&vec!["02:orchestrator".to_string()]]).await.unwrap();
    let old_queue:Uuid=client.query_one("INSERT INTO reborn_validation_queue(tenant_id,user_id,agent_id,project_id,component_id,component_class,original_binding_required) VALUES($1,$2,$3,$4,$5,22,false) RETURNING id",
        &[&sc.tenant_id,&sc.user_id,&sc.agent_id,&sc.project_id,&other]).await.unwrap().get(0);
    drop(client);
    let mut input = proposal(Uuid::new_v4(), other, document("historical-code"));
    input
        .as_object_mut()
        .unwrap()
        .remove("create_queue_project");
    input["queue_ids"] = json!([old_queue]);
    let (status, _) = super::tests::send(&app, "POST", URI, Some(&input), true).await;
    assert!(!status.is_success());
    let rows: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM reborn_component_revisions WHERE component_id=$1",
            &[&other],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        rows, 0,
        "failed capture rolls back allocation and submission"
    );
}

#[tokio::test]
async fn original_queue_upgrade_preserves_old_pending_bytes_without_backfill() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE SCHEMA original_binding_upgrade; SET search_path TO original_binding_upgrade,public").await.unwrap();
    client
        .batch_execute(concat!(
            include_str!("../../brassclaw_pg/migrations/V051__reborn_validation_queue.sql"),
            include_str!(
                "../../brassclaw_pg/migrations/V078__reborn_validation_queue_q2_actor.sql"
            ),
            include_str!("../../brassclaw_pg/migrations/V097__component_q1_review_seals.sql"),
        ))
        .await
        .unwrap();
    let component = Uuid::new_v4();
    let before:String=client.query_one("INSERT INTO reborn_validation_queue(tenant_id,user_id,agent_id,project_id,component_id,component_class,counter,review_feedback,proposed_payload) VALUES('t','u','a','p',$1,22,2,'preserve original feedback','{\"content\":\"original proposed source\"}') RETURNING to_jsonb(reborn_validation_queue)::text",&[&component]).await.unwrap().get(0);
    client
        .batch_execute(include_str!(
            "../../brassclaw_pg/migrations/V206__original_review_queue_bindings.sql"
        ))
        .await
        .unwrap();
    let row=client.query_one("SELECT (to_jsonb(q)-'original_binding_required')::text,original_binding_required,state FROM reborn_validation_queue q WHERE component_id=$1",&[&component]).await.unwrap();
    assert_eq!(row.get::<_, String>(0), before);
    assert!(!row.get::<_, bool>(1));
    assert_eq!(row.get::<_, i16>(2), 1);
    let new_required:bool=client.query_one("INSERT INTO reborn_validation_queue(tenant_id,user_id,agent_id,project_id,component_id,component_class) VALUES('t','u','a','p',$1,22) RETURNING original_binding_required",&[&Uuid::new_v4()]).await.unwrap().get(0);
    assert!(new_required);
    let backfilled: i64 = client
        .query_one(
            "SELECT count(*) FROM reborn_component_review_queue_bindings",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(backfilled, 0);
    client
        .batch_execute("SET search_path TO public")
        .await
        .unwrap();
}

#[tokio::test]
async fn original_queue_escaped_source_near_revision_limit_roundtrips_without_seal_quota() {
    use brassclaw_skills::component_revision::REVISION_LIMITS;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let app = super::tests::app(rig.pool.clone());
    let component = Uuid::new_v4();
    let operation = Uuid::new_v4();
    let line = "# source: \"quoted\" \\directory\\file \"tail\"\n";
    let mut doc = document("escaped-original");
    // Size against the existing revision envelope owner, including its JSON
    // escaping. No independent binding quota is introduced by this fixture.
    doc["content"] = json!(line);
    let sample = proposal(operation, component, doc.clone());
    let envelope_len = sample["candidate_bytes"].as_str().unwrap().len();
    let encoded_line = serde_json::to_string(line).unwrap().len() - 2;
    let count = (REVISION_LIMITS.max_bytes - envelope_len) / encoded_line;
    doc["content"] = json!(line.repeat(count));
    let mut input = proposal(operation, component, doc.clone());
    let original = input["candidate_bytes"].as_str().unwrap().to_owned();
    assert!(original.len() <= REVISION_LIMITS.max_bytes);
    assert!(REVISION_LIMITS.max_bytes - original.len() < encoded_line * 2);
    // The existing operator route's 14-MiB transport limit remains in force.
    assert!(serde_json::to_vec(&input).unwrap().len() < 14 * 1024 * 1024);
    // A real migrated row may also carry formatter output, which is not
    // revision input and has no 64-MiB combined-seal policy. Keep this on the
    // server side: do not broaden the existing HTTP/revision input contract.
    let sc = scope();
    let store = ValidationQueueStore::new(rig.pool.clone());
    let client = rig.pool.get().await.unwrap();
    client.execute("INSERT INTO reborn_python_code(id,tenant_id,user_id,agent_id,project_id,name,description,content,consumer_tags) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        &[&component,&sc.tenant_id,&sc.user_id,&sc.agent_id,&sc.project_id,&doc["name"].as_str().unwrap(),&doc["description"].as_str().unwrap(),&doc["content"].as_str().unwrap(),&vec!["02:orchestrator".to_string()]]).await.unwrap();
    let cache_unit = r#"\""#;
    let repeats = i32::try_from(REVISION_LIMITS.max_bytes / cache_unit.len()).unwrap();
    client
        .execute(
            "UPDATE reborn_python_code SET formatted_content=repeat($2::text,$3::int) WHERE id=$1",
            &[&component, &cache_unit, &repeats],
        )
        .await
        .unwrap();
    drop(client);
    store.submit(&sc, component, 22, None).await.unwrap();
    let queue_id = store.list(&sc, None).await.unwrap()[0].id;
    input
        .as_object_mut()
        .unwrap()
        .remove("create_queue_project");
    input["queue_ids"] = json!([queue_id]);
    let (status, receipt) = super::tests::send(&app, "POST", URI, Some(&input), true).await;
    assert!(status.is_success(), "{status}: {receipt}");
    let seal_bytes:i32=rig.pool.get().await.unwrap().query_one("SELECT octet_length(binding_bytes) FROM reborn_component_review_queue_bindings WHERE queue_id=$1",&[&queue_id]).await.unwrap().get(0);
    assert!(
        seal_bytes > 64 * 1024 * 1024,
        "regression: legal expanded seal must exceed the removed arbitrary ceiling"
    );
    let captured = ValidationQueueStore::new(rig.pool.clone())
        .capture_q1_candidate(&scope(), component)
        .await
        .unwrap();
    let live: Value = serde_json::from_str(captured.original().unwrap().component_bytes()).unwrap();
    assert_eq!(live["content"], doc["content"]);
    let original_subject = PgReviewSubmissionStore::new(rig.pool.clone())
        .read(operation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        original_subject.snapshot.revisions()[&component]
            .draft()
            .exact_bytes(),
        original
    );
    let (status, retry) = super::tests::send(&app, "POST", URI, Some(&input), true).await;
    assert!(status.is_success());
    assert_eq!(receipt, retry);
}

#[tokio::test]
async fn original_queue_http_projects_unresolved_historical_and_bound_states_without_mutation() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let app = super::tests::app(rig.pool.clone());
    let sc = scope();
    let store = ValidationQueueStore::new(rig.pool.clone());
    let legacy = Uuid::new_v4();
    rig.pool.get().await.unwrap().execute("INSERT INTO reborn_recipes(id,tenant_id,user_id,agent_id,project_id,name,description,steps) VALUES($1,$2,$3,$4,$5,'legacy-unresolved-recipe','Preserved draft','[]')",
        &[&legacy,&sc.tenant_id,&sc.user_id,&sc.agent_id,&sc.project_id]).await.unwrap();
    store.submit(&sc, legacy, 21, None).await.unwrap();
    let historical = Uuid::new_v4();
    let client = rig.pool.get().await.unwrap();
    client.execute("INSERT INTO reborn_recipes(id,tenant_id,user_id,agent_id,project_id,name,description,steps) VALUES($1,$2,$3,$4,$5,'historical-custody-recipe','Preserved old draft','[]')",
        &[&historical,&sc.tenant_id,&sc.user_id,&sc.agent_id,&sc.project_id]).await.unwrap();
    client.execute("INSERT INTO reborn_validation_queue(tenant_id,user_id,agent_id,project_id,component_id,component_class,original_binding_required) VALUES($1,$2,$3,$4,$5,21,false)",
        &[&sc.tenant_id,&sc.user_id,&sc.agent_id,&sc.project_id,&historical]).await.unwrap();
    drop(client);
    let component = Uuid::new_v4();
    let input = proposal(Uuid::new_v4(), component, document("bound-code-projection"));
    let (status, receipt) = super::tests::send(&app, "POST", URI, Some(&input), true).await;
    assert!(status.is_success());
    let before: String = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT to_jsonb(q)::text FROM reborn_validation_queue q WHERE component_id=$1",
            &[&legacy],
        )
        .await
        .unwrap()
        .get(0);
    let uri = "/api/webchat/v2/validation-queue?project_id=default&q=auto";
    let (status, view) = super::tests::send(&app, "GET", uri, None, true).await;
    assert!(status.is_success(), "{status}: {view}");
    let rows = view["items"].as_array().unwrap();
    let old = rows.iter().find(|r| r["id"] == legacy.to_string()).unwrap();
    assert_eq!(old["original_review"]["state"], "unresolved");
    assert!(old["original_review"]["queue_id"].is_string());
    assert!(old["original_review"]["submission_id"].is_null());
    let bound = rows
        .iter()
        .find(|r| r["id"] == component.to_string())
        .unwrap();
    assert_eq!(bound["original_review"]["state"], "bound");
    assert_eq!(
        bound["original_review"]["submission_id"],
        receipt["submission_id"]
    );
    let (status, count) = super::tests::send(
        &app,
        "GET",
        "/api/webchat/v2/validation-queue/count?project_id=default&status=pending",
        None,
        true,
    )
    .await;
    assert!(status.is_success());
    assert_eq!(count["count"], 3);
    let after: String = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT to_jsonb(q)::text FROM reborn_validation_queue q WHERE component_id=$1",
            &[&legacy],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(before, after, "diagnosis must not rewrite queue/evidence");
    let old = view["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == historical.to_string())
        .unwrap();
    assert_eq!(old["original_review"]["state"], "legacy_unresolved");
}

#[tokio::test]
async fn original_queue_broken_transport_preserves_unavailable_and_original_readback() {
    use brassclaw_pg::test_support::{AckCutRule, AckCutTransport};
    use brassclaw_product_workflow::RecipeStoreError;
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let app = super::tests::app(rig.pool.clone());
    let component = Uuid::new_v4();
    let operation = Uuid::new_v4();
    let input = proposal(operation, component, document("transport-original"));
    let (status, receipt) = super::tests::send(&app, "POST", URI, Some(&input), true).await;
    assert!(status.is_success());
    let draft = ReviewSubmissionDraft::new(
        operation,
        &scope().user_id,
        ComponentRevisionDraft::from_json(input["candidate_bytes"].as_str().unwrap()).unwrap(),
        None,
        vec![],
    )
    .unwrap();
    let transport =
        AckCutTransport::start(rig.url.expose_secret(), vec![AckCutRule::next_commit(1)])
            .await
            .unwrap();
    let pool = transport.pool.clone();
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    // Close the real wire while the original read transaction is alive.
    transport.shutdown().await.unwrap();
    let result = ValidationQueueStore::read_original_create(&tx, &draft, &scope()).await;
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("disconnected original read must fail"),
    };
    assert!(matches!(
        crate::validation_queue::original_review_error(error),
        RecipeStoreError::Unavailable(_)
    ));
    let queue_id: Uuid = receipt["queue_ids"][0].as_str().unwrap().parse().unwrap();
    let error = match queue::read_in_transaction(&tx, queue_id).await {
        Err(error) => error,
        Ok(_) => panic!("disconnected binding read must fail"),
    };
    assert!(matches!(
        crate::validation_queue::original_review_error(
            crate::validation_queue::map_original_store(error, component)
        ),
        RecipeStoreError::Unavailable(_)
    ));
    drop(tx);
    drop(client);
    let (status, retry) = super::tests::send(&app, "POST", URI, Some(&input), true).await;
    assert!(status.is_success());
    assert_eq!(receipt, retry);
}

#[tokio::test]
async fn original_queue_v1_projection_inventories_every_current_class_column() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let client = rig.pool.get().await.unwrap();
    for class in [0_i16, 1, 2, 3, 13, 21, 22, 23] {
        // Inventory against the migrated database, not a second Rust field list.
        // A new semantic column must deliberately version the projection owner;
        // it cannot silently disappear or reinterpret an old immutable seal.
        let row=client.query_one("SELECT review_queue_fields($1::smallint),
            (SELECT array_agg(a.attname::text ORDER BY a.attnum) FROM pg_attribute a
             WHERE a.attrelid=to_regclass(review_queue_table($1::smallint)) AND a.attnum>0 AND NOT a.attisdropped
               AND review_component_bytes(jsonb_build_object(a.attname,true))::jsonb ? a.attname::text
               AND NOT a.attname::text=ANY(review_queue_derived_fields()))",&[&class]).await.unwrap();
        let mut declared: Vec<String> = row.get(0);
        let mut actual: Vec<String> = row.get(1);
        declared.sort();
        actual.sort();
        assert_eq!(
            declared, actual,
            "class {class} has unclassified or missing projection fields"
        );
    }
}

#[tokio::test]
async fn original_queue_tool_skill_and_binding_capture_rejects_missing_and_mismatched_metadata() {
    use brassclaw_skills::{
        review_submission_store::FunctionReviewPartnerDraft,
        revision_store::PgComponentRevisionStore,
    };
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let sc = scope();
    let store = ValidationQueueStore::new(rig.pool.clone());
    let revisions = PgComponentRevisionStore::new(rig.pool.clone());
    for class in [0_i16, 1, 2, 3, 13] {
        let root = Uuid::new_v4();
        let tool = Uuid::new_v4();
        let binding = Uuid::new_v4();
        let code = Uuid::new_v4();
        // These exact retained prerequisites are deliberately unreviewed context,
        // not fake executable/Q1 evidence. Only original capture is under test.
        let primitive = |id, class, deps: Vec<Uuid>| {
            ComponentRevisionDraft::from_json(
                &json!({"format":"component-revision/1","uuid":id,"class_code":class,
            "document":{},"dependencies":deps,"association":null})
                .to_string(),
            )
            .unwrap()
        };
        let tool_ref = revisions
            .stage(&primitive(tool, 0, vec![]), 0)
            .await
            .unwrap();
        let binding_ref = revisions
            .stage(&primitive(binding, 13, vec![tool]), 0)
            .await
            .unwrap();
        let code_ref = revisions
            .stage(&primitive(code, 22, vec![]), 0)
            .await
            .unwrap();
        let table = crate::validation_queue::resolve_component_table(i32::from(class)).unwrap();
        let name = format!("original-class-{class}-{}", root.simple());
        let client = rig.pool.get().await.unwrap();
        client.execute(&format!("INSERT INTO {table}(id,tenant_id,user_id,agent_id,project_id,name,description,class_code) VALUES($1,$2,$3,$4,$5,$6,'Exact original metadata fixture',$7)"),
            &[&root,&sc.tenant_id,&sc.user_id,&sc.agent_id,&sc.project_id,&name,&class]).await.unwrap();
        let registry = match class {
            0 => json!({"helper":code}),
            13 => json!({"tool":tool}),
            _ => json!({"tool":tool,"binding":binding,"code":code}),
        };
        client.execute(&format!("UPDATE {table} SET dependency_registry=$2,prior_knowledge_content='Exact original guidance',override_prompt_creation=true WHERE id=$1"),&[&root,&registry]).await.unwrap();
        if (1..=3).contains(&class) {
            let compatibility = match class {
                1 => "brassclaw-class:rusty",
                2 => "brassclaw-class:monty",
                _ => "brassclaw-class:llm",
            };
            client.execute("UPDATE reborn_skills SET body='Original usage',compatibility=$2,license='MIT',allowed_tools=ARRAY['host.read_file'],version='1.2.3',
                keywords=ARRAY['original'],exclude_keywords=ARRAY['excluded'],patterns=ARRAY['exact'],tags=ARRAY['review'],max_context_tokens=4096,
                setup_marker='ready',required_binaries=ARRAY['read-helper'],required_env=ARRAY['ORIGINAL_ENV'],required_config=ARRAY['read.mode'] WHERE id=$1",&[&root,&compatibility]).await.unwrap();
        } else if class == 0 {
            client.execute("UPDATE reborn_tools SET capability_id='host.read_file',param_schema='{\"path\":\"string\"}',param_template='{\"path\":\"original\"}' WHERE id=$1",&[&root]).await.unwrap();
        } else {
            client.execute("UPDATE reborn_tool_skills SET tool_name='host.read_file',content='Original binding',param_schema='{\"path\":\"string\"}',param_template='{\"path\":\"original\"}' WHERE id=$1",&[&root]).await.unwrap();
        }
        let document:Value=client.query_one(&format!("SELECT review_queue_projection($2::smallint,to_jsonb(c)) FROM {table} c WHERE id=$1"),&[&root,&class]).await.unwrap().get(0);
        drop(client);
        store
            .submit(&sc, root, i32::from(class), None)
            .await
            .unwrap();
        let queued = store
            .list(&sc, None)
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.component_id == root)
            .unwrap();
        let queue_id = queued.id;
        let association = if (1..=3).contains(&class) {
            Some(json!({"format":"skill-association/1","skill_uuid":root,"python_code_uuid":code,"tool_skill_uuid":binding,"tool_uuid":tool,
            "callable":"host.read_file","inputs":{},"arguments":{},"code_arguments":{},"result":{"type":"string"},
            "failure":{"action":"stop","max_attempts":1,"idempotency":"not_assumed","idempotency_evidence_ref":null,"retryable_outcomes":[]}}).to_string())
        } else {
            None
        };
        let declared = match class {
            0 => vec![code],
            13 => vec![tool],
            _ => vec![tool, binding, code],
        };
        let selected = match class {
            0 => vec![code_ref],
            13 => vec![tool_ref],
            _ => vec![tool_ref, binding_ref],
        };
        let make = |doc: Value| {
            let candidate = ComponentRevisionDraft::from_json(
                &json!({"format":"component-revision/1","uuid":root,"class_code":class,
                "document":doc,"dependencies":declared,"association":association})
                .to_string(),
            )
            .unwrap();
            if (1..=3).contains(&class) {
                ReviewSubmissionDraft::new_function(
                    Uuid::new_v4(),
                    "operator",
                    candidate,
                    None,
                    selected.clone(),
                    0,
                    FunctionReviewPartnerDraft::Context {
                        reference: code_ref,
                    },
                )
                .unwrap()
            } else {
                ReviewSubmissionDraft::new(
                    Uuid::new_v4(),
                    "operator",
                    candidate,
                    None,
                    selected.clone(),
                )
                .unwrap()
            }
        };
        let mut mismatches = Vec::new();
        let mut missing = document.clone();
        missing
            .as_object_mut()
            .unwrap()
            .remove("dependency_registry");
        mismatches.push(missing);
        for (field, value) in [
            ("dependency_registry", json!({"other":"wrong"})),
            ("prior_knowledge_content", json!("wrong usage")),
            ("override_prompt_creation", json!(false)),
            ("replaces_id", json!(Uuid::new_v4())),
        ] {
            let mut bad = document.clone();
            bad[field] = value;
            mismatches.push(bad);
        }
        let specialized: Vec<(&str, Value)> = match class {
            0 => vec![
                ("capability_id", json!("host.other")),
                ("param_schema", json!({})),
                ("effect_type", json!("write")),
            ],
            13 => vec![
                ("tool_name", json!("host.other")),
                ("param_schema", json!({})),
                ("includes", json!([Uuid::new_v4()])),
            ],
            _ => vec![
                ("compatibility", json!("wrong")),
                ("allowed_tools", json!([])),
                ("license", json!("other")),
                ("version", json!("2.0.0")),
                ("required_binaries", json!([])),
                ("required_env", json!([])),
                ("required_config", json!([])),
                ("setup_marker", Value::Null),
                ("max_context_tokens", json!(1)),
            ],
        };
        for (field, value) in specialized {
            let mut bad = document.clone();
            bad[field] = value;
            mismatches.push(bad);
        }
        let original_queue: String = rig
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT to_jsonb(q)::text FROM reborn_validation_queue q WHERE id=$1",
                &[&queue_id],
            )
            .await
            .unwrap()
            .get(0);
        for bad in mismatches {
            let draft = make(bad);
            let mut client = rig.pool.get().await.unwrap();
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .start()
                .await
                .unwrap();
            let result = ValidationQueueStore::submit_original_in_transaction(
                &tx,
                &draft,
                &[(sc.clone(), root, queue_id)],
                &[queue_id],
                "existing_queue",
            )
            .await;
            assert!(
                matches!(result, Err(ValidationQueueError::ReviewChanged { .. })),
                "class {class} must reject incomplete/different original metadata"
            );
            tx.rollback().await.unwrap();
        }
        // Independently challenge the DB trigger: a legitimate immutable
        // subject and correctly checksummed seal still cannot certify different
        // actual authored metadata, even when bypassing the caller precheck.
        let mut bad_document = document.clone();
        bad_document["dependency_registry"] = json!({"wrong":"different original"});
        let bad_original = make(bad_document);
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        let queue_bytes:String=tx.query_one("SELECT review_queue_bytes(q) FROM reborn_validation_queue q WHERE id=$1 FOR UPDATE",&[&queue_id]).await.unwrap().get(0);
        let component_bytes:String=tx.query_one(&format!("SELECT review_component_bytes(to_jsonb(c)) FROM {table} c WHERE id=$1 FOR UPDATE"),&[&root]).await.unwrap().get(0);
        let bad_subject = PgReviewSubmissionStore::submit_in_transaction(&tx, &bad_original)
            .await
            .unwrap();
        tx.execute("INSERT INTO reborn_component_review_queue_requests(submission_id,queue_ids,request_kind) VALUES($1,$2,'existing_queue')",&[&bad_subject.id,&vec![queue_id]]).await.unwrap();
        let forged = queue::retain_in_transaction(
            &tx,
            &bad_subject,
            queue_id,
            root,
            component_bytes.clone(),
            queue_bytes,
            component_bytes,
        )
        .await;
        assert!(
            matches!(
                forged,
                Err(brassclaw_skills::review_submission_store::SubmissionStoreError::Database(_))
            ),
            "real binding trigger must reject class {class} metadata mismatch"
        );
        tx.rollback().await.unwrap();
        drop(client);
        let client = rig.pool.get().await.unwrap();
        let row=client.query_one("SELECT (SELECT count(*) FROM reborn_component_revisions WHERE component_id=$1),to_jsonb(q)::text FROM reborn_validation_queue q WHERE id=$2",&[&root,&queue_id]).await.unwrap();
        assert_eq!(row.get::<_, i64>(0), 0);
        assert_eq!(row.get::<_, String>(1), original_queue);
        drop(client);
        let original = make(document);
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        let subject = ValidationQueueStore::submit_original_in_transaction(
            &tx,
            &original,
            &[(sc.clone(), root, queue_id)],
            &[queue_id],
            "existing_queue",
        )
        .await
        .unwrap();
        assert_eq!(subject.candidate.class_code, i32::from(class));
        if (1..=3).contains(&class) {
            assert!(
                subject
                    .function_partner
                    .is_some_and(|p| !p.changed && p.reference == code_ref)
            );
        }
        tx.commit().await.unwrap();
        drop(client);
        let captured = store.capture_q1_candidate(&sc, root).await.unwrap();
        assert_eq!(captured.original().unwrap().submission_id(), subject.id);
    }
}

#[tokio::test]
async fn original_queue_http_pages_full_recipe_stock_without_class_starvation() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let app = super::tests::app(rig.pool.clone());
    let sc = scope();
    let size = crate::pg_recipe_store::MAX_RECIPE_LIST_ROWS;
    let client = rig.pool.get().await.unwrap();
    client.execute("INSERT INTO reborn_recipes(tenant_id,user_id,agent_id,project_id,name,description,steps,created_at)
        SELECT $1,$2,$3,$4,'page-recipe-'||i,'Pending original needed','[]',TIMESTAMPTZ '2000-01-01 00:00:00+00'+i*interval '1 second' FROM generate_series(1,$5::bigint) i",
        &[&sc.tenant_id,&sc.user_id,&sc.agent_id,&sc.project_id,&size]).await.unwrap();
    let early = Uuid::new_v4();
    let late = Uuid::new_v4();
    for (component, name, time) in [
        (early, "page-early-code", "1999-01-01T00:00:00Z"),
        (late, "page-late-code", "2002-01-01T00:00:00Z"),
    ] {
        client.execute("INSERT INTO reborn_python_code(id,tenant_id,user_id,agent_id,project_id,name,description) VALUES($1,$2,$3,$4,$5,$6,'Unresolved original')",
            &[&component,&sc.tenant_id,&sc.user_id,&sc.agent_id,&sc.project_id,&name]).await.unwrap();
        client.execute("INSERT INTO reborn_validation_queue(tenant_id,user_id,agent_id,project_id,component_id,component_class,submitted_at)
            VALUES($1,$2,$3,$4,$5,22,$6::text::timestamptz)",&[&sc.tenant_id,&sc.user_id,&sc.agent_id,&sc.project_id,&component,&time]).await.unwrap();
    }
    drop(client);
    let uri = "/api/webchat/v2/validation-queue?project_id=default&q=auto";
    let (status, first) = super::tests::send(&app, "GET", uri, None, true).await;
    assert!(status.is_success(), "{status}: {first}");
    assert_eq!(first["page_size"], size);
    assert_eq!(first["items"].as_array().unwrap().len() as i64, size);
    assert_eq!(
        first["items"][0]["id"],
        early.to_string(),
        "global ordering must precede Recipe-only ordering"
    );
    let after = first["next_cursor"]
        .as_str()
        .expect("full page must expose remaining rows");
    let (status, second) =
        super::tests::send(&app, "GET", &format!("{uri}&after={after}"), None, true).await;
    assert!(status.is_success(), "{status}: {second}");
    assert_eq!(second["items"].as_array().unwrap().len(), 2);
    assert!(second["next_cursor"].is_null());
    assert_eq!(
        second["items"][1]["id"],
        late.to_string(),
        "later non-Recipe original must be reachable"
    );
    assert_eq!(second["items"][1]["original_review"]["state"], "unresolved");
    let all = first["items"]
        .as_array()
        .unwrap()
        .iter()
        .chain(second["items"].as_array().unwrap())
        .collect::<Vec<_>>();
    let unique = all
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len() as i64, size + 2);
    for pair in all.windows(2) {
        let key = |row: &Value| {
            (
                row["queued_at"].as_str().unwrap().to_owned(),
                row["class_code"].as_u64().unwrap(),
                row["id"].as_str().unwrap().to_owned(),
            )
        };
        assert!(
            key(pair[0]) < key(pair[1]),
            "all classes/pages share one strict oldest-first order"
        );
    }
    let (status, _) = super::tests::send(
        &app,
        "GET",
        &format!("/api/webchat/v2/validation-queue?project_id=default&q=manual&after={after}"),
        None,
        true,
    )
    .await;
    assert_eq!(
        status,
        axum::http::StatusCode::BAD_REQUEST,
        "a cursor cannot silently switch queue buckets"
    );
}

#[tokio::test]
async fn original_queue_rejection_api_retains_writer_deletion_candidates_and_counts() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let app = super::tests::app(rig.pool.clone());
    let sc = scope();
    let writer = ValidationQueueStore::new(rig.pool.clone());
    let threshold = i32::from(writer.reject_threshold());
    let recipe = Uuid::new_v4();
    let code = Uuid::new_v4();
    let client = rig.pool.get().await.unwrap();
    client.execute("INSERT INTO reborn_recipes(id,tenant_id,user_id,agent_id,project_id,name,description,steps) VALUES($1,$2,$3,$4,$5,'terminal-recipe','Rejection lifecycle fixture','[]')",
        &[&recipe,&sc.tenant_id,&sc.user_id,&sc.agent_id,&sc.project_id]).await.unwrap();
    client.execute("INSERT INTO reborn_python_code(id,tenant_id,user_id,agent_id,project_id,name,description) VALUES($1,$2,$3,$4,$5,'terminal-code','Rejection lifecycle fixture')",
        &[&code,&sc.tenant_id,&sc.user_id,&sc.agent_id,&sc.project_id]).await.unwrap();
    drop(client);
    for (id, class) in [(recipe, 21), (code, 22)] {
        writer.submit(&sc, id, class, None).await.unwrap();
        // Seed only the attempt count before the existing lifecycle fixture owner
        // captures the real candidate/queue seals. This is no Q1 approval evidence.
        rig.pool
            .get()
            .await
            .unwrap()
            .execute(
                "UPDATE reborn_validation_queue SET counter=$2 WHERE component_id=$1 AND state=1",
                &[&id, &(threshold - 1)],
            )
            .await
            .unwrap();
        writer.gate1_pass(&sc, id, &[]).await.unwrap();
        writer.reject(&sc, id, "Final rejection").await.unwrap();
    }
    let rows = writer.list(&sc, None).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.state == 4 && r.counter == threshold));
    let uri = "/api/webchat/v2/validation-queue?project_id=default&q=rejection";
    let (status, view) = super::tests::send(&app, "GET", uri, None, true).await;
    assert!(status.is_success(), "{status}: {view}");
    let items = view["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    for item in items {
        assert_eq!(item["validation_status"], "garbage");
        assert_eq!(item["queue_code"], "garbage");
        assert_eq!(item["review_attempts"], threshold);
    }
    let (status, revision) = super::tests::send(
        &app,
        "GET",
        "/api/webchat/v2/validation-queue?project_id=default&q=revision",
        None,
        true,
    )
    .await;
    assert!(status.is_success());
    assert!(revision["items"].as_array().unwrap().is_empty());
    for (status_name, expected) in [("garbage", 2), ("rejected", 0), ("pending", 0)] {
        let (status, count) = super::tests::send(
            &app,
            "GET",
            &format!(
                "/api/webchat/v2/validation-queue/count?project_id=default&status={status_name}"
            ),
            None,
            true,
        )
        .await;
        assert!(status.is_success());
        assert_eq!(count["count"], expected);
    }
    // Listing deletion candidates must not weaken the original/Q1 approval gate.
    let (status, _) = super::tests::send(
        &app,
        "PUT",
        &format!("/api/webchat/v2/components/21/{recipe}/validate?project_id=default"),
        Some(&json!({})),
        true,
    )
    .await;
    assert!(!status.is_success());
    assert!(
        writer
            .list(&sc, None)
            .await
            .unwrap()
            .iter()
            .all(|r| r.state == 4)
    );
}
