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
    assert!(!status.is_success());
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
            "intent_examples":null,"dependency_registry":null});
        if class == 21 {
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
