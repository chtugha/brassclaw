//! Actual factory/ACK/HTTP/PG graph acceptance. No approval or provider result
//! is invented. Larger graphs remain unreviewed component data.
use super::*;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use brassclaw_host_api::{UserId, task_preparation::RetainedPreparation};
use brassclaw_pg::{
    PgPool,
    test_support::{AckBoundary, AckCutRule, AckCutTransport, AckHoldEnd},
};
use brassclaw_product_workflow::{
    MontyMemoryMode, MontyVmSettingsStore, RebornServices, SubmitComponentReviewRequest,
};
use brassclaw_skills::{
    association_contract::ComponentRevisionRef,
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS},
    review_submission_store::{PgReviewSubmissionStore, ReviewSubmissionDraft},
    revision_store::{PgComponentRevisionStore, RevisionStoreError, SnapshotReadLimits},
};
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio_postgres::IsolationLevel;
use tower::ServiceExt;
use uuid::Uuid;

const URI: &str = "/api/webchat/v2/component-review-submissions";
struct Operator(UserId);
#[async_trait::async_trait]
impl crate::WebuiAuthenticator for Operator {
    async fn authenticate(&self, token: &str) -> Option<UserId> {
        (token == "k01-test-operator").then(|| self.0.clone())
    }
    fn allows_operator_webui_config(&self) -> bool {
        true
    }
}
async fn start(rig: &crate::runtime::test_pg::PgRig, home: &std::path::Path) -> Arc<RebornRuntime> {
    rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
    Arc::new(
        Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
            rig.build_input("k01-snapshot", home)
                .with_runtime_policy(local_dev_runtime_policy()),
        )))
        .await
        .unwrap(),
    )
}
async fn app(runtime: &Arc<RebornRuntime>) -> Router {
    crate::webui_v2_app(
        crate::build_webui_services(runtime.clone(), None)
            .await
            .unwrap(),
        crate::WebuiServeConfig::new(
            runtime.thread_scope.tenant_id.clone(),
            Arc::new(Operator(runtime.actor_user_id.clone())),
            vec![],
        ),
    )
    .unwrap()
}
fn wire_app(runtime: &RebornRuntime, pool: Arc<PgPool>) -> Router {
    // Same actual facade/store owners as composition, with the existing wire
    // fault transport's pool so only this authoring connection is instrumented.
    let api = RebornServices::new(
        runtime.thread_service.clone(),
        Arc::new(brassclaw_turns::DefaultTurnCoordinator::new(Arc::new(
            brassclaw_turns::PgTurnStateStore::new(
                pool.clone(),
                runtime.thread_scope.tenant_id.as_str(),
            ),
        ))),
    )
    .with_recipe_store(Arc::new(
        crate::pg_recipe_store::PgRecipeStoreFacade::new(
            pool,
            runtime.thread_scope.tenant_id.as_str(),
            runtime.thread_scope.agent_id.as_str(),
        )
        .with_catalogue(runtime.installed_catalogue.clone()),
    ));
    crate::webui_v2_app(
        crate::RebornWebuiBundle {
            api: Arc::new(api),
            product_auth: None,
            readiness: crate::RebornReadiness::disabled(),
        },
        crate::WebuiServeConfig::new(
            runtime.thread_scope.tenant_id.clone(),
            Arc::new(Operator(runtime.actor_user_id.clone())),
            vec![],
        ),
    )
    .unwrap()
}
async fn send(app: &Router, method: &str, uri: &str, body: Option<&Value>) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("Authorization", "Bearer k01-test-operator");
    let body = match body {
        Some(v) => {
            builder = builder.header("Content-Type", "application/json");
            v.to_string()
        }
        None => String::new(),
    };
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 512 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
fn revision(id: Uuid, dependencies: &[Uuid], payload: &str) -> ComponentRevisionDraft {
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":23,"document":{"payload":payload},"dependencies":dependencies,"association":null}).to_string()).unwrap()
}
fn exact_max_revision(id: Uuid, quotes: bool) -> ComponentRevisionDraft {
    let overhead = revision(id, &[], "").exact_bytes().len();
    let remaining = REVISION_LIMITS.max_bytes - overhead;
    let payload = if quotes {
        format!(
            "{}{}",
            "\"".repeat(remaining / 2),
            "a".repeat(remaining % 2)
        )
    } else {
        "a".repeat(remaining)
    };
    let draft = revision(id, &[], &payload);
    assert_eq!(draft.exact_bytes().len(), REVISION_LIMITS.max_bytes);
    draft
}
fn request(
    id: Uuid,
    candidate: &ComponentRevisionDraft,
    dependencies: &[ComponentRevisionRef],
) -> Value {
    serde_json::to_value(SubmitComponentReviewRequest {
        queue_ids: vec![],
        create_queue_project: None,
        submission_id: id.to_string(),
        candidate_bytes: candidate.exact_bytes().into(),
        expected_previous_version: Some(0),
        base: None,
        dependencies: dependencies
            .iter()
            .copied()
            .map(crate::pg_review_submission::selection)
            .collect(),
        function_partner: None,
    })
    .unwrap()
}
async fn acknowledge(
    runtime: &RebornRuntime,
    count: u32,
    graph_bytes: u64,
    value_bytes: u64,
) -> RetainedPreparation {
    let store = runtime.webui_monty_settings_store();
    let before = store.get("default", "default").await.unwrap();
    let mut limits = before.execution_limits;
    limits.max_catalogue_components_per_recipe = count;
    limits.max_catalogue_bytes = graph_bytes;
    limits.max_value_bytes = value_bytes;
    limits.max_value_nodes = 2_000_000;
    limits.max_ipc_frame_bytes = 256 * 1024 * 1024;
    limits.utility_worker_memory_bytes = 640 * 1024 * 1024;
    let update = serde_json::from_value(
        json!({"expected_revision":before.revision,"execution_limits":limits}),
    )
    .unwrap();
    let saved = store.upsert("default", "default", &update).await.unwrap();
    let captured = runtime
        .installed_catalogue
        .capture_task_preparation()
        .await
        .unwrap();
    assert_eq!(captured.revision(), saved.revision);
    assert_eq!(captured.policy(), saved.execution_limits);
    captured.original().clone()
}
async fn retain_leaves(
    pool: &Arc<PgPool>,
    count: usize,
    bytes: usize,
) -> Vec<ComponentRevisionRef> {
    let mut client =
        crate::pg_transaction_client::TransactionClient::from_client(pool.get().await.unwrap());
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let payload = "x".repeat(bytes);
    let mut refs = Vec::new();
    for _ in 0..count {
        refs.push(
            PgComponentRevisionStore::stage_in_transaction(
                &tx,
                &revision(Uuid::new_v4(), &[], &payload),
                0,
            )
            .await
            .unwrap(),
        );
    }
    tx.commit().await.unwrap();
    client.recycle_after_commit();
    refs
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_k01_acknowledged_http_graphs_above_fixed_caps_preserve_originals() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let app = app(&runtime).await;
    let original = acknowledge(&runtime, 5001, 160 * 1024 * 1024, 256 * 1024 * 1024).await;
    // A declared large body and an actual unbounded chunk stream must be
    // rejected by the real operator auth before any poll or capture request.
    for declared in [false, true] {
        use tracing::instrument::WithSubscriber;
        let polls = Arc::new(AtomicUsize::new(0));
        let observed_polls = polls.clone();
        let stream = futures::stream::poll_fn(move |_| {
            observed_polls.fetch_add(1, Ordering::SeqCst);
            std::task::Poll::<Option<Result<Vec<u8>, std::io::Error>>>::Pending
        });
        let mut builder = Request::builder()
            .method("POST")
            .uri(URI)
            .header("Content-Type", "application/json");
        if declared {
            builder = builder.header("Content-Length", 100 * 1024 * 1024);
        }
        let counts = Batches::default();
        let response = tokio::time::timeout(
            Duration::from_secs(30),
            app.clone()
                .oneshot(builder.body(Body::from_stream(stream)).unwrap())
                .with_subscriber(counts.clone()),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            polls.load(Ordering::SeqCst),
            0,
            "unauthenticated body must remain unread"
        );
        assert_eq!(
            counts.2.load(Ordering::SeqCst),
            0,
            "auth rejection must not reach preparation/store owner"
        );
    }
    let leaves = retain_leaves(&rig.pool, 5000, 1).await;
    let root = revision(
        Uuid::new_v4(),
        &leaves.iter().map(|r| r.uuid).collect::<Vec<_>>(),
        "complete graph",
    );
    let id = Uuid::new_v4();
    let input = request(id, &root, &leaves);
    let (status, receipt) = send(&app, "POST", URI, Some(&input)).await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["preparation_revision"], original.revision());
    let first = PgReviewSubmissionStore::new(rig.pool.clone())
        .read(id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.snapshot.revisions().len(), 5001);
    assert_eq!(
        serde_json::from_str::<Value>(&first.subject_bytes).unwrap()["format"],
        "component-review-submission/4"
    );
    // New admission shrinks, original graph/capture and original-ID retry do not.
    acknowledge(&runtime, 5000, 160 * 1024 * 1024, 256 * 1024 * 1024).await;
    assert_eq!(
        send(&app, "POST", URI, Some(&input)).await,
        (StatusCode::OK, receipt.clone())
    );
    let mut different_id = input.clone();
    different_id["submission_id"] = json!(Uuid::new_v4());
    assert!(
        !send(&app, "POST", URI, Some(&different_id))
            .await
            .0
            .is_success()
    );
    let (_, view) = send(&app, "GET", &format!("{URI}/{id}"), None).await;
    assert_eq!(view["subject_bytes"], first.subject_bytes);
    assert_eq!(view["component_bytes"].as_array().unwrap().len(), 5001);
    let mut changed = input.clone();
    changed["candidate_bytes"] = json!(
        revision(
            root.uuid(),
            &leaves.iter().map(|r| r.uuid).collect::<Vec<_>>(),
            "changed same ID"
        )
        .exact_bytes()
    );
    assert_eq!(
        send(&app, "POST", URI, Some(&changed)).await.0,
        StatusCode::CONFLICT
    );
    acknowledge(&runtime, 6200, 160 * 1024 * 1024, 256 * 1024 * 1024).await;
    let large = retain_leaves(&rig.pool, 65, 1024 * 1024).await;
    let large_root = revision(
        Uuid::new_v4(),
        &large.iter().map(|r| r.uuid).collect::<Vec<_>>(),
        "over64MiB",
    );
    let large_id = Uuid::new_v4();
    let (status, value) = send(
        &app,
        "POST",
        URI,
        Some(&request(large_id, &large_root, &large)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    let saved = PgReviewSubmissionStore::new(rig.pool.clone())
        .read(large_id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        saved
            .snapshot
            .revisions()
            .values()
            .map(|r| r.draft().exact_bytes().len())
            .sum::<usize>()
            > 64 * 1024 * 1024
    );
    // New /4 function subjects count BOTH roots and exercise the actual V193
    // successor partner trigger, retaining no approval or Tool registration.
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let (tool, binding) = crate::pg_review_submission::function_tests::prerequisites(&store).await;
    let code = ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":Uuid::new_v4(),"class_code":22,"document":{"content":"def value(): return 'data'"},"dependencies":[],"association":null}).to_string()).unwrap();
    let usage = crate::pg_review_submission::function_tests::skill(
        Uuid::new_v4(),
        code.uuid(),
        binding.uuid,
        tool.uuid,
        "Unreviewed function data",
    );
    let function_id = Uuid::new_v4();
    let mut function_input = request(function_id, &usage, &[tool, binding]);
    function_input["function_partner"] = json!({"role":"changed","candidate_bytes":code.exact_bytes(),"base":null,"expected_previous_version":0});
    acknowledge(&runtime, 3, 160 * 1024 * 1024, 256 * 1024 * 1024).await;
    assert!(
        !send(&app, "POST", URI, Some(&function_input))
            .await
            .0
            .is_success()
    );
    assert!(
        store
            .read_allocation_head(usage.uuid())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .read_allocation_head(code.uuid())
            .await
            .unwrap()
            .is_none()
    );
    acknowledge(&runtime, 4, 160 * 1024 * 1024, 256 * 1024 * 1024).await;
    let (status, function_receipt) = send(&app, "POST", URI, Some(&function_input)).await;
    assert_eq!(status, StatusCode::OK, "{function_receipt}");
    assert_eq!(function_receipt["function_partner"]["changed"], true);
    let function = PgReviewSubmissionStore::new(rig.pool.clone())
        .read(function_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(function.snapshot.revisions().len(), 4);
    assert!(function.preparation.is_some());
    let conflicting_usage = crate::pg_review_submission::function_tests::skill(
        Uuid::from_u128(1),
        code.uuid(),
        binding.uuid,
        tool.uuid,
        "second-root conflict",
    );
    let mut conflict = request(Uuid::new_v4(), &conflicting_usage, &[tool, binding]);
    conflict["function_partner"] = function_input["function_partner"].clone();
    assert_eq!(
        send(&app, "POST", URI, Some(&conflict)).await.0,
        StatusCode::CONFLICT
    );
    assert!(
        store
            .read_allocation_head(conflicting_usage.uuid())
            .await
            .unwrap()
            .is_none()
    );
    acknowledge(&runtime, 6200, 160 * 1024 * 1024, 256 * 1024 * 1024).await;
    // Escaping lifts the actual HTTP body above14MiB and subject above16MiB.
    let escaped = exact_max_revision(Uuid::new_v4(), true);
    let escaped_id = Uuid::new_v4();
    let escaped_input = request(escaped_id, &escaped, &[]);
    assert!(escaped_input.to_string().len() > 14 * 1024 * 1024);
    assert!(
        send(&app, "POST", URI, Some(&escaped_input))
            .await
            .0
            .is_success()
    );
    let escaped_saved = PgReviewSubmissionStore::new(rig.pool.clone())
        .read(escaped_id)
        .await
        .unwrap()
        .unwrap();
    assert!(escaped_saved.subject_bytes.len() > 16 * 1024 * 1024);
    // Exact old per-revision maximum retains room for117 bytes of SQL framing.
    let legacy = exact_max_revision(Uuid::new_v4(), false);
    let old_id = Uuid::new_v4();
    let old = PgReviewSubmissionStore::new(rig.pool.clone())
        .submit(
            &ReviewSubmissionDraft::new(
                old_id,
                runtime.actor_user_id.as_str(),
                legacy,
                None,
                vec![],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    acknowledge(&runtime, 1, 1024, 8192).await;
    let old_read = PgReviewSubmissionStore::new(rig.pool.clone())
        .read(old_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old_read.subject_bytes, old.subject_bytes);
    assert_eq!(
        old_read
            .snapshot
            .revisions()
            .values()
            .next()
            .unwrap()
            .draft()
            .exact_bytes()
            .len(),
        REVISION_LIMITS.max_bytes
    );
    let oversized = Request::builder()
        .method("POST")
        .uri(URI)
        .header("Authorization", "Bearer k01-test-operator")
        .header("Content-Type", "application/json")
        .body(Body::from_stream(futures::stream::iter([Ok::<
            _,
            std::io::Error,
        >(
            vec![b'x'; 8193],
        )])))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(oversized).await.unwrap().status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    drop(app);
    drop(first);
    drop(saved);
    drop(escaped_saved);
    shutdown_shared_runtime(runtime).await.unwrap();
}

#[derive(Clone, Default)]
struct Batches(Arc<AtomicUsize>, Arc<AtomicUsize>, Arc<AtomicUsize>);
struct Phase<'a>(&'a Batches);
impl tracing::field::Visit for Phase<'_> {
    fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "phase" {
            if value == "metadata" {
                self.0.0.fetch_add(1, Ordering::SeqCst);
            }
            if value == "content" {
                self.0.1.fetch_add(1, Ordering::SeqCst);
            }
            if value == "capture" {
                self.0.2.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
}
impl tracing::Subscriber for Batches {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        matches!(
            metadata.target(),
            "brassclaw::snapshot_read" | "brassclaw::component_preparation"
        )
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        event.record(&mut Phase(self));
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_k01_real_streamed_batches_keep_complete_membership_and_original_limits() {
    use tracing::instrument::WithSubscriber;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let mut client =
        crate::pg_transaction_client::TransactionClient::from_client(rig.pool.get().await.unwrap());
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let mut refs = Vec::new();
    let mut previous = None;
    for _ in 0..5001 {
        let dependencies: Vec<_> = previous.into_iter().collect();
        let draft = revision(Uuid::new_v4(), &dependencies, "chain");
        let reference = PgComponentRevisionStore::stage_in_transaction(&tx, &draft, 0)
            .await
            .unwrap();
        previous = Some(reference.uuid);
        refs.push(reference);
    }
    tx.commit().await.unwrap();
    client.recycle_after_commit();
    drop(client);
    let capture = acknowledge(&runtime, 6200, 160 * 1024 * 1024, 8192).await;
    let limits = SnapshotReadLimits::captured(&capture).unwrap();
    let counts = Batches::default();
    let observed = counts.clone();
    let root = previous.unwrap();
    let snapshot = PgComponentRevisionStore::new(rig.pool.clone())
        .read_exact(&[root], &refs, limits)
        .with_subscriber(observed)
        .await
        .unwrap();
    assert_eq!(snapshot.revisions().len(), refs.len());
    assert!(counts.0.load(Ordering::SeqCst) > 1);
    assert_eq!(
        counts.0.load(Ordering::SeqCst),
        counts.1.load(Ordering::SeqCst)
    );
    acknowledge(&runtime, 1, 1024, 8192).await;
    // The original explicit graph profile survives a later acknowledged decrease.
    assert_eq!(
        PgComponentRevisionStore::new(rig.pool.clone())
            .read_exact(&[root], &refs, limits)
            .await
            .unwrap()
            .revisions()
            .len(),
        refs.len()
    );
    let now = runtime
        .installed_catalogue
        .capture_task_preparation()
        .await
        .unwrap();
    assert!(matches!(
        PgComponentRevisionStore::new(rig.pool.clone())
            .read_exact(
                &[root],
                &refs,
                SnapshotReadLimits::captured(now.original()).unwrap()
            )
            .await,
        Err(RevisionStoreError::Capacity)
    ));
    let mut incomplete = refs.clone();
    incomplete.remove(0);
    assert!(
        PgComponentRevisionStore::new(rig.pool.clone())
            .read_exact(&[root], &incomplete, limits)
            .await
            .is_err()
    );
    let mut duplicate = refs.clone();
    duplicate.push(refs[0]);
    assert!(
        PgComponentRevisionStore::new(rig.pool.clone())
            .read_exact(&[root], &duplicate, limits)
            .await
            .is_err()
    );
    shutdown_shared_runtime(runtime).await.unwrap();
}

async fn backend(pool: &PgPool) -> i32 {
    pool.get()
        .await
        .unwrap()
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_k01_real_http_pool_one_held_begin_commit_and_unknown_ack_keep_original() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let timeout = Duration::from_millis(
        runtime
            .installed_catalogue
            .preparation_policy()
            .await
            .unwrap()
            .response_timeout_millis,
    );
    for boundary in [AckBoundary::Begin, AckBoundary::Commit] {
        for cancel in [false, true] {
            let id = Uuid::new_v4();
            let mut rule = AckCutRule::after_bind(
                1,
                if boundary == AckBoundary::Begin {
                    vec![id.as_bytes().to_vec()]
                } else {
                    vec![
                        id.as_bytes().to_vec(),
                        b"component-review-submission/4".to_vec(),
                    ]
                },
                vec![],
                boundary,
            )
            .hold();
            if boundary == AckBoundary::Begin {
                rule = rule.following_query(
                    b"START TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY\0".to_vec(),
                );
            }
            let wire = AckCutTransport::start(rig.url.expose_secret(), vec![rule])
                .await
                .unwrap();
            wire.pool.resize(1);
            let pid = backend(&wire.pool).await;
            let mut ticket = wire.arm_hold(1, pid).unwrap();
            if boundary == AckBoundary::Begin {
                wire.pool
                    .get()
                    .await
                    .unwrap()
                    .query_one("SELECT $1::uuid", &[&id])
                    .await
                    .unwrap();
            }
            let app = wire_app(&runtime, wire.pool.clone());
            let input = request(id, &revision(Uuid::new_v4(), &[], "held original"), &[]);
            let calling_app = app.clone();
            let calling_input = input.clone();
            let caller = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
                send(&calling_app, "POST", URI, Some(&calling_input)).await
            }));
            let held = tokio::time::timeout(timeout, ticket.wait_held())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(held.backend_pid, pid);
            assert_eq!(held.boundary, boundary);
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
                    .get::<_, bool>(0)
            );
            if cancel {
                caller.abort();
                assert!(caller.await.unwrap_err().is_cancelled());
                assert_eq!(
                    tokio::time::timeout(timeout, ticket.wait_closed())
                        .await
                        .unwrap()
                        .unwrap(),
                    AckHoldEnd::PeerClosed
                );
                assert!(ticket.release().await.is_err());
                assert_ne!(
                    tokio::time::timeout(timeout, backend(&wire.pool))
                        .await
                        .unwrap(),
                    pid
                );
                let exists = PgReviewSubmissionStore::new(rig.pool.clone())
                    .read(id)
                    .await
                    .unwrap()
                    .is_some();
                assert_eq!(exists, boundary == AckBoundary::Commit);
                if exists {
                    assert!(send(&app, "POST", URI, Some(&input)).await.0.is_success());
                    assert_eq!(rig.pool.get().await.unwrap().query_one("SELECT count(*) FROM reborn_component_review_submissions WHERE submission_id=$1", &[&id]).await.unwrap().get::<_, i64>(0), 1);
                }
            } else {
                ticket.release().await.unwrap();
                assert!(
                    tokio::time::timeout(timeout, caller)
                        .await
                        .unwrap()
                        .unwrap()
                        .0
                        .is_success()
                );
                assert_eq!(
                    backend(&wire.pool).await,
                    pid,
                    "confirmed success must recycle normally"
                );
            }
            drop(app);
            wire.shutdown().await.unwrap();
        }
    }
    // A real cut after committed subject bytes forces one fresh exact readback.
    let id = Uuid::new_v4();
    let wire = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![
                id.as_bytes().to_vec(),
                b"component-review-submission/4".to_vec(),
            ],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    wire.pool.resize(1);
    wire.arm(1).unwrap();
    let app = wire_app(&runtime, wire.pool.clone());
    let input = request(id, &revision(Uuid::new_v4(), &[], "lost ACK original"), &[]);
    let first = send(&app, "POST", URI, Some(&input)).await;
    assert_eq!(first.0, StatusCode::OK, "{}", first.1);
    assert_eq!(wire.cut_count(), 1);
    assert_eq!(send(&app, "POST", URI, Some(&input)).await, first);
    drop(app);
    wire.shutdown().await.unwrap();
    shutdown_shared_runtime(runtime).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_k01_desired_without_ack_and_streamed_http_keep_first_effective_capture() {
    let mut rig = crate::runtime::test_pg::pg_rig().await;
    let observer = rig.pool.clone();
    let wire = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![
            AckCutRule::after_bind(
                1,
                vec![
                    b"max_catalogue_components_per_recipe".to_vec(),
                    b"5001".to_vec(),
                ],
                vec![],
                AckBoundary::Commit,
            )
            .hold(),
        ],
    )
    .await
    .unwrap();
    Arc::get_mut(&mut rig)
        .expect("unshared fixture before factory construction")
        .pool = wire.pool.clone();
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let app = app(&runtime).await;
    let settings = runtime.webui_monty_settings_store();
    let before = settings.get("default", "default").await.unwrap();
    let old_capture = runtime
        .installed_catalogue
        .capture_task_preparation()
        .await
        .unwrap();
    let mut limits = before.execution_limits;
    limits.max_catalogue_components_per_recipe = 5001;
    let update = serde_json::from_value(
        json!({"expected_revision":before.revision,"execution_limits":limits}),
    )
    .unwrap();
    wire.pool.resize(1);
    let pid = backend(&wire.pool).await;
    let mut held = wire.arm_hold(1, pid).unwrap();
    let editing = {
        let settings = settings.clone();
        tokio::spawn(async move { settings.upsert("default", "default", &update).await })
    };
    let timeout = Duration::from_millis(before.execution_limits.settings_uptake_timeout_millis);
    let observation = tokio::time::timeout(timeout, held.wait_held())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(observation.backend_pid, pid);
    let desired = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
        observer.clone(),
        "default",
        "default",
    )
    .get("default", "default")
    .await
    .unwrap();
    assert_eq!(desired.revision, before.revision + 1);
    assert_eq!(
        desired.execution_limits.max_catalogue_components_per_recipe,
        5001
    );
    let during = runtime
        .installed_catalogue
        .capture_task_preparation()
        .await
        .unwrap();
    assert_eq!(
        during.bytes(),
        old_capture.bytes(),
        "persisted desired is not worker ACK"
    );
    // Accept a NEW real HTTP request while desired is committed but unacked.
    // Its first body poll proves auth/rate/capture have finished; block only the
    // client stream, then let the actual settings ACK complete before content.
    let id = Uuid::new_v4();
    let input = request(id, &revision(Uuid::new_v4(), &[], "pending ACK"), &[]).to_string();
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let body = {
        let entered = entered.clone();
        let release = release.clone();
        Body::from_stream(futures::stream::once(async move {
            entered.notify_one();
            release.notified().await;
            Ok::<_, std::io::Error>(input)
        }))
    };
    let caller_app = app.clone();
    let calling = tokio::spawn(async move {
        let response = caller_app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(URI)
                    .header("Authorization", "Bearer k01-test-operator")
                    .header("Content-Type", "application/json")
                    .body(body)
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        (status, serde_json::from_slice::<Value>(&body).unwrap())
    });
    tokio::time::timeout(timeout, entered.notified())
        .await
        .unwrap();
    held.release().await.unwrap();
    let applied = editing.await.unwrap().unwrap();
    let new_capture = runtime
        .installed_catalogue
        .capture_task_preparation()
        .await
        .unwrap();
    assert_eq!(new_capture.revision(), applied.revision);
    assert_eq!(new_capture.policy(), limits);
    release.notify_one();
    let (status, receipt) = calling.await.unwrap();
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["preparation_revision"], before.revision);
    let saved = PgReviewSubmissionStore::new(observer)
        .read(id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        saved.preparation.as_ref().unwrap().bytes(),
        old_capture.bytes()
    );
    drop(app);
    drop(settings);
    shutdown_shared_runtime(runtime).await.unwrap();
    wire.shutdown().await.unwrap();
}
