//! The existing operator authoring route and real PG wire. These retain
//! unreviewed class-21/23 drafts; no VM, native Q1 or approval is manufactured.
use super::*;
use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport, AckHoldEnd};
use brassclaw_product_workflow::MontyVmSettingsStore;
use secrecy::ExposeSecret;
use std::time::Duration;

fn request(class: i32, id: Uuid, component: Uuid) -> Value {
    let mut document = projection_document(class);
    document["name"] = json!(format!("queued-custody-{class}-{component}"));
    let candidate = json!({"format":"component-revision/1","uuid":component,"class_code":class,
        "document":document,"dependencies":[],"association":null})
    .to_string();
    json!({"submission_id":id,"candidate_bytes":candidate,"base":null,"dependencies":[],"create_queue_project":"default"})
}
async fn deadline(pool: &Arc<PgPool>) -> Duration {
    let configured = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
        pool.clone(),
        "default",
        "default",
    )
    .get("default", "default")
    .await
    .unwrap();
    Duration::from_millis(configured.execution_limits.response_timeout_millis)
}
async fn backend(pool: &brassclaw_pg::PgPool) -> i32 {
    pool.get()
        .await
        .unwrap()
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0)
}
async fn submit(
    app: &axum::Router,
    input: &Value,
    timeout: Duration,
) -> (axum::http::StatusCode, Value) {
    tokio::time::timeout(
        timeout,
        super::super::tests::send(app, "POST", URI, Some(input), true),
    )
    .await
    .unwrap()
}
async fn assert_original(pool: &Arc<PgPool>, id: Uuid, component: Uuid, input: &Value) {
    let original = PgReviewSubmissionStore::new(pool.clone())
        .read(id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        original.snapshot.revisions()[&component]
            .draft()
            .exact_bytes(),
        input["candidate_bytes"].as_str().unwrap()
    );
    let row = pool.get().await.unwrap().query_one("SELECT
        (SELECT count(*) FROM reborn_component_revisions WHERE component_id=$2),
        (SELECT count(*) FROM reborn_component_review_submissions WHERE submission_id=$1),
        (SELECT cardinality(queue_ids) FROM reborn_component_review_queue_requests WHERE submission_id=$1),
        (SELECT count(*) FROM reborn_validation_queue WHERE component_id=$2 AND state=1),
        (SELECT count(*) FROM reborn_validation_queue WHERE component_id=$2 AND state<>1),
        (SELECT count(*) FROM brassclaw_turns WHERE record_kind='private_work' AND validator_submission_id=$1),
        (SELECT count(*) FROM reborn_validator_q1_runs WHERE submission_id=$1),
        (SELECT count(*) FROM brassclaw_monty_tool_invocations)", &[&id,&component]).await.unwrap();
    assert_eq!(row.get::<_, i64>(0), 1);
    assert_eq!(row.get::<_, i64>(1), 1);
    assert_eq!(row.get::<_, i32>(2), 1);
    assert_eq!(row.get::<_, i64>(3), 1);
    for column in 4..8 {
        assert_eq!(
            row.get::<_, i64>(column),
            0,
            "authoring custody grants no Work, Q1 or Tool execution"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_queue_recipe_and_catalogue_commit_ack_loss_uses_exact_original() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let timeout = deadline(&rig.pool).await;
    for class in [21, 23] {
        let (id, component) = (Uuid::new_v4(), Uuid::new_v4());
        let wire = AckCutTransport::start(
            rig.url.expose_secret(),
            vec![
                AckCutRule::after_bind(
                    1,
                    vec![id.as_bytes().to_vec(), b"create_queue".to_vec()],
                    vec![],
                    AckBoundary::Commit,
                ),
                AckCutRule::after_bind(
                    2,
                    vec![
                        b"brassclaw.validation-queue".to_vec(),
                        component.to_string().into_bytes(),
                    ],
                    vec![],
                    AckBoundary::Commit,
                ),
            ],
        )
        .await
        .unwrap();
        wire.pool.resize(1);
        let app = super::super::tests::app(wire.pool.clone());
        let input = request(class, id, component);
        wire.arm(1).unwrap();
        let (status, receipt) = submit(&app, &input, timeout).await;
        assert!(
            status.is_success(),
            "class {class}: actual create-COMMIT loss must read its original: {status}: {receipt}"
        );
        assert_eq!(wire.cut_count(), 1);
        assert_eq!(receipt["catalogue_activated"], false);
        assert_eq!(receipt["review_status"], "unreviewed");
        assert_original(&rig.pool, id, component, &input).await;
        // Successful early original read returns its acknowledged connection
        // normally; the guard must not discard every successful transaction.
        let pid = backend(&wire.pool).await;
        let (status, replay) = submit(&app, &input, timeout).await;
        assert!(status.is_success());
        assert_eq!(replay, receipt);
        assert_eq!(backend(&wire.pool).await, pid);
        // Match the queue lock Bind after identity preflight. A lost COMMIT
        // response on that existing-original queue branch is unavailable,
        // never a second create. Its next same-ID read is exact.
        wire.arm(2).unwrap();
        let (status, _) = submit(&app, &input, timeout).await;
        assert_eq!(status, axum::http::StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(wire.cut_count(), 2);
        let (status, replay) = submit(&app, &input, timeout).await;
        assert!(status.is_success());
        assert_eq!(replay, receipt);
        assert_original(&rig.pool, id, component, &input).await;
        wire.shutdown().await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_queue_recipe_and_catalogue_held_ack_discards_cancelled_callers() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let timeout = deadline(&rig.pool).await;
    for class in [21, 23] {
        for existing in [false, true] {
            let (id, component) = (Uuid::new_v4(), Uuid::new_v4());
            let wire = AckCutTransport::start(
                rig.url.expose_secret(),
                vec![
                    AckCutRule::after_bind(
                        1,
                        vec![
                            b"brassclaw.validation-queue".to_vec(),
                            component.to_string().into_bytes(),
                        ],
                        vec![],
                        AckBoundary::Commit,
                    )
                    .hold(),
                ],
            )
            .await
            .unwrap();
            wire.pool.resize(1);
            let app = super::super::tests::app(wire.pool.clone());
            let input = request(class, id, component);
            if existing {
                let before = backend(&wire.pool).await;
                assert!(submit(&app, &input, timeout).await.0.is_success());
                assert_eq!(
                    backend(&wire.pool).await,
                    before,
                    "confirmed fresh create must recycle its connection"
                );
            }
            let pid = backend(&wire.pool).await;
            // Match the queue lock Bind in the actual create/existing
            // transaction; the earlier identity-only read must pass normally.
            let mut ticket = wire.arm_hold(1, pid).unwrap();
            let caller_app = app.clone();
            let caller_input = input.clone();
            let caller = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
                super::super::tests::send(&caller_app, "POST", URI, Some(&caller_input), true).await
            }));
            let held = tokio::time::timeout(timeout, ticket.wait_held())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(held.backend_pid, pid);
            assert!(
                rig.pool
                    .get()
                    .await
                    .unwrap()
                    .query_one(
                        "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1)",
                        &[&pid]
                    )
                    .await
                    .unwrap()
                    .get::<_, bool>(0),
                "the instrument must leave this actual backend open until the caller is dropped"
            );
            assert_original(&rig.pool, id, component, &input).await;
            caller.abort();
            assert!(caller.await.unwrap_err().is_cancelled());
            assert_eq!(
                tokio::time::timeout(timeout, ticket.wait_closed())
                    .await
                    .unwrap()
                    .unwrap(),
                AckHoldEnd::PeerClosed
            );
            assert!(
                ticket.release().await.is_err(),
                "no wire release/cut may substitute for physical caller disposal"
            );
            let replacement = tokio::time::timeout(timeout, backend(&wire.pool))
                .await
                .expect("max-one pool must free the old caller's slot");
            assert_ne!(replacement, pid);
            tokio::time::timeout(timeout, async {
                loop {
                    let live: bool = rig
                        .pool
                        .get()
                        .await
                        .unwrap()
                        .query_one(
                            "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1)",
                            &[&pid],
                        )
                        .await
                        .unwrap()
                        .get(0);
                    if !live {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("the old physical PG connection must actually end");
            assert!(submit(&app, &input, timeout).await.0.is_success());
            assert_original(&rig.pool, id, component, &input).await;
            wire.shutdown().await.unwrap();
        }
    }
}
