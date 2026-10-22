//! Actual operator API, PostgreSQL, global Monty and retained native dispatcher.
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft, revision_store::PgComponentRevisionStore,
};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

struct CatalogueOperator;
#[async_trait::async_trait]
impl crate::WebuiAuthenticator for CatalogueOperator {
    async fn authenticate(&self, token: &str) -> Option<UserId> {
        (token == "catalogue-acceptance").then(|| UserId::new("catalogue-operator").unwrap())
    }
    fn allows_operator_webui_config(&self) -> bool {
        true
    }
}
async fn call(
    app: &axum::Router,
    method: &str,
    body: Option<&Value>,
    authenticated: bool,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri("/api/webchat/v2/catalogue")
        .header("Content-Type", "application/json");
    if authenticated {
        req = req.header("Authorization", "Bearer catalogue-acceptance");
    }
    let response = app
        .clone()
        .oneshot(
            req.body(body.map_or(Body::empty(), |v| Body::from(v.to_string())))
                .unwrap(),
        )
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
fn reference(r: brassclaw_skills::association_contract::ComponentRevisionRef) -> Value {
    json!({"uuid":r.uuid,"class_code":r.class_code,"version":r.version,"checksum":hex::encode(r.checksum)})
}

#[tokio::test]
async fn native_catalogue_reference_upgrade_preserves_legacy_receipts() {
    use secrecy::ExposeSecret;

    const PIN_VERSION: u32 = 174;
    let rig = super::super::test_pg::pg_rig().await;
    // This is a separate empty database on the fixture's private native server,
    // not a hand-written approximation of an old schema or a shared database.
    let database = format!("catalogue_pin_upgrade_{}", Uuid::new_v4().simple());
    rig.pool
        .get()
        .await
        .unwrap()
        .batch_execute(&format!(
            "CREATE DATABASE \"{database}\" TEMPLATE template0"
        ))
        .await
        .unwrap();
    let mut endpoint = url::Url::parse(rig.url.expose_secret()).unwrap();
    endpoint.set_path(&format!("/{database}"));
    let pool = brassclaw_pg::pool::build_pool(endpoint.as_str()).unwrap();
    let mut client = pool.get().await.unwrap();
    client.batch_execute("SET TIME ZONE 'UTC'").await.unwrap();
    let runner = brassclaw_pg::migrations::migrations::runner();
    let previous = runner
        .get_migrations()
        .iter()
        .map(|migration| migration.version())
        .filter(|version| *version < PIN_VERSION)
        .max()
        .unwrap();
    runner
        .set_target(refinery::Target::Version(previous))
        .run_async(&mut **client)
        .await
        .unwrap();
    let mut runs = Vec::new();
    for index in 0..3_u8 {
        let run = Uuid::new_v4();
        let scope = brassclaw_turns::TurnScope::new_with_owner(
            brassclaw_host_api::TenantId::new("catalogue-upgrade").unwrap(),
            Some(brassclaw_host_api::AgentId::new("upgrade-agent").unwrap()),
            None,
            brassclaw_host_api::ThreadId::new(format!("legacy-{index}")).unwrap(),
            Some(UserId::new("upgrade-owner").unwrap()),
        );
        client
            .execute(
                "INSERT INTO brassclaw_monty_task_admissions
                 (run_id,turn_id,scope,accepted_message_ref,runner_id,claim_checksum,admission_key,phase)
                 VALUES($1,$2,$3,$4,$5,$6,$7,'reserved')",
                &[
                    &run,
                    &Uuid::new_v4(),
                    &serde_json::to_value(scope).unwrap(),
                    &format!("msg:{}", Uuid::new_v4()),
                    &Uuid::new_v4(),
                    &vec![index + 1; 32],
                    &vec![index + 10; 32],
                ],
            )
            .await
            .unwrap();
        if index > 0 {
            client
                .execute(
                    "UPDATE brassclaw_monty_task_admissions
                     SET phase='started',started_at=clock_timestamp() WHERE run_id=$1",
                    &[&run],
                )
                .await
                .unwrap();
        }
        if index == 2 {
            client
                .execute(
                    "UPDATE brassclaw_monty_task_admissions SET phase='settled',
                     settled_at=clock_timestamp(),outcome=$2
                     WHERE run_id=$1",
                    &[
                        &run,
                        &json!({"status":"failed","reason_kind":"task_cancelled"}),
                    ],
                )
                .await
                .unwrap();
        }
        runs.push(run);
    }
    // Hash the full old rows instead of printing private admission keys in a
    // failed assertion. Only the new, explicitly unknown fields are excluded.
    let before: std::collections::BTreeMap<Uuid, String> = client
        .query(
            "SELECT run_id,encode(sha256(convert_to(to_jsonb(a)::text,'UTF8')),'hex')
             FROM brassclaw_monty_task_admissions a",
            &[],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect();
    brassclaw_pg::migrations::migrations::runner()
        .set_target(refinery::Target::Version(PIN_VERSION))
        .run_async(&mut **client)
        .await
        .unwrap();
    let rows = client
        .query(
            "SELECT run_id,catalogue_reference_version,catalogue_id,catalogue_checksum,
                    encode(sha256(convert_to((to_jsonb(a) - ARRAY[
                        'catalogue_reference_version','catalogue_id','catalogue_checksum'])::text,'UTF8')),'hex')
             FROM brassclaw_monty_task_admissions a",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 3);
    for row in rows {
        assert_eq!(row.get::<_, i16>(1), 0);
        assert_eq!(row.get::<_, Option<Uuid>>(2), None);
        assert_eq!(row.get::<_, Option<String>>(3), None);
        assert_eq!(before[&row.get::<_, Uuid>(0)], row.get::<_, String>(4));
    }
    for run in runs {
        let error = client
            .execute(
                "UPDATE brassclaw_monty_task_admissions
                 SET catalogue_reference_version=1 WHERE run_id=$1",
                &[&run],
            )
            .await
            .unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_authored_catalogue_activates_validated_usage_without_revalidation() {
    let rig = super::super::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let root = tempfile::tempdir().unwrap();
    let input = RebornRuntimeInput::from_services(
        rig.build_input("authored-catalogue-owner", root.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )
    .with_model_gateway_override(Arc::new(RecordingGateway {
        reply: "unmatched reply".into(),
        requests: Arc::new(StdMutex::new(Vec::new())),
    }))
    .with_poll_settings(PollSettings {
        interval: Duration::from_millis(10),
        max_total: RUNTIME_SEND_TIMEOUT,
    });
    let runtime = Arc::new(Box::pin(build_reborn_runtime(input)).await.unwrap());
    let catalogue = runtime.installed_catalogue.clone();
    let installed = catalogue.reply.program().inputs().instruction();
    assert!(
        !installed.association_approvals().is_empty(),
        "trusted usage must be qualified before this test"
    );
    let old = catalogue.active.read().await.clone();
    let new_id = Uuid::new_v4();
    let original = &installed.snapshot().revisions()[&installed.recipe().uuid];
    let mut document = original.draft().document().clone();
    document["name"] = json!("authored-library-reply");
    document["mcp_call"]["name"] = json!("authored_reply");
    document["mcp_call"]["sentence"] = json!("authored reply %");
    for field in ["examples", "negative_examples"] {
        for example in document["mcp_call"][field].as_array_mut().unwrap() {
            *example = json!(
                example
                    .as_str()
                    .unwrap()
                    .replace("publish literal reply", "authored reply")
            );
        }
    }
    for field in ["success_examples", "failure_examples"] {
        for case in document["mcp_qualification"][field].as_array_mut().unwrap() {
            case["command"] = json!(
                case["command"]
                    .as_str()
                    .unwrap()
                    .replace("publish literal reply", "authored reply")
            );
        }
    }
    document["variants"][0]["intent_examples"] = json!(["authored reply %"]);
    let mut alternative = document["variants"][0].clone();
    alternative["variant_key"] = json!("alternative");
    alternative["intent_examples"] = json!(["alternative authored reply %"]);
    document["variants"]
        .as_array_mut()
        .unwrap()
        .push(alternative);
    document["input_layouts"]["alternative"] =
        document["input_layouts"][installed.variant().variant_key.as_str()].clone();
    let draft = ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":new_id,"class_code":21,"document":document,"dependencies":original.draft().dependencies(),"association":null}).to_string()).unwrap();
    let revised = PgComponentRevisionStore::new(rig.pool.clone())
        .stage(&draft, 0)
        .await
        .unwrap();
    let mut refs: Vec<_> = installed
        .snapshot()
        .revisions()
        .values()
        .filter(|r| r.reference().uuid != installed.recipe().uuid)
        .map(|r| reference(r.reference()))
        .collect();
    refs.push(reference(revised));
    let api = crate::build_webui_services(runtime.clone(), None)
        .await
        .unwrap();
    let app = crate::webui_v2_app(
        api,
        crate::WebuiServeConfig::new(
            TenantId::new(runtime.webui_tenant_id()).unwrap(),
            Arc::new(CatalogueOperator),
            Vec::new(),
        ),
    )
    .unwrap();
    let (status, initial) = call(&app, "GET", None, true).await;
    assert_eq!(status, StatusCode::OK);
    // Exact installed Tool usages remain qualified, but adding validator
    // metadata cannot bypass its private coverage contract by omitting the tag.
    // Preparation invokes no Recipe steps and cannot publish either draft.
    for (tagged, empty_variants, expected) in [
        (false, false, "private validator tags required"),
        (true, false, "invalid validator coverage declaration"),
        (true, true, "activated Recipe requires a selected variant"),
    ] {
        let invalid_id = Uuid::new_v4();
        let mut document = draft.document().clone();
        document["validates_class_code"] = json!(22);
        if tagged {
            document["consumer_tags"] = json!(["05:validator"]);
            for variant in document["variants"].as_array_mut().unwrap() {
                variant["intent_examples"] = json!([]);
            }
        } else {
            document.as_object_mut().unwrap().remove("consumer_tags");
        }
        if empty_variants {
            document["variants"] = json!([]);
        }
        let invalid_draft = ComponentRevisionDraft::from_json(
            &json!({
                "format":"component-revision/1","uuid":invalid_id,"class_code":21,
                "document":document,"dependencies":draft.dependencies(),"association":null
            })
            .to_string(),
        )
        .unwrap();
        let invalid_revision = PgComponentRevisionStore::new(rig.pool.clone())
            .stage(&invalid_draft, 0)
            .await
            .unwrap();
        let mut components = refs.clone();
        *components.last_mut().unwrap() = reference(invalid_revision);
        let selection = serde_json::from_value(json!({
            "recipe_id":invalid_id,"workflow_class":"deterministic",
            "components":components,"association_approvals":{}
        }))
        .unwrap();
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        let result = catalogue
            .prepare_authored(
                &tx,
                &[selection],
                catalogue.preparation_policy().await.unwrap(),
            )
            .await;
        assert!(
            matches!(result, Err(brassclaw_product_workflow::RecipeStoreError::Invalid(ref reason))
            if reason == expected),
            "validator declaration must fail for its precise contract violation"
        );
        tx.rollback().await.unwrap();
        assert_eq!(catalogue.active.read().await.generation, old.generation);
    }
    let activation = Uuid::new_v4();
    let request = json!({"activation_id":activation,"expected_generation":initial["generation"],"recipes":[{"recipe_id":new_id,"workflow_class":"deterministic","components":refs,"association_approvals":{}}]});
    assert_eq!(
        call(&app, "POST", Some(&request), false).await.0,
        StatusCode::UNAUTHORIZED
    );
    let client = rig.pool.get().await.unwrap();
    let evidence_before: i64 = client
        .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
        .await
        .unwrap()
        .get(0);
    let (status, receipt) = call(&app, "POST", Some(&request), true).await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["catalogue_activated"], true);
    let discovery = runtime.mcp_recipe_discovery().snapshot().unwrap();
    assert_eq!(json!(discovery.generation()), receipt["generation"]);
    assert_eq!(
        discovery.tools_list(),
        json!({"tools":[]}),
        "new command remains unadvertised pending ordinary full-command evidence"
    );

    assert_eq!(
        client
            .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        evidence_before,
        "activation must reuse approvals, not revalidate usages"
    );
    assert_eq!(
        call(&app, "POST", Some(&request), true).await.1,
        receipt,
        "unknown commit retry is idempotent"
    );
    let effects_at_activation: i64 = client
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get(0);
    catalogue.restore_authored().await.unwrap();
    assert_eq!(
        json!(catalogue.active.read().await.generation),
        receipt["generation"],
        "startup restoration reconstructs the exact authored selection"
    );
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        effects_at_activation,
        "restoration must not execute the Recipe"
    );
    let selected = catalogue.active.read().await.clone();
    assert_eq!(old.programs.len(), 0);
    assert_eq!(selected.programs.len(), 2);
    let chat = runtime.new_conversation().await.unwrap();
    let reply = runtime
        .send_user_message(&chat, "authored reply quoted ' ü {{literal}}")
        .await
        .unwrap();
    assert_eq!(reply.status, TurnStatus::Completed);
    assert_eq!(reply.text.as_deref(), Some("quoted ' ü {{literal}}"));
    let alternative = runtime
        .send_user_message(&chat, "alternative authored reply second variant")
        .await
        .unwrap();
    assert_eq!(alternative.status, TurnStatus::Completed);
    assert_eq!(alternative.text.as_deref(), Some("second variant"));
    let outcome: Value = client.query_one("SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'", &[&reply.run_id.as_uuid()]).await.unwrap().get(0);
    assert!(
        outcome.to_string().contains(&new_id.to_string()),
        "actual normal match retains authored selection"
    );
    let effects_before: i64 = client
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get(0);
    let mut stale = request.clone();
    stale["activation_id"] = json!(Uuid::new_v4());
    assert!(
        call(&app, "POST", Some(&stale), true)
            .await
            .0
            .is_client_error(),
        "stale catalogue CAS fails"
    );
    let mut forged = request.clone();
    forged["activation_id"] = json!(Uuid::new_v4());
    forged["expected_generation"] = receipt["generation"].clone();
    forged["recipes"][0]["association_approvals"] = json!(std::collections::BTreeMap::from([(
        Uuid::new_v4().to_string(),
        Uuid::new_v4()
    )]));
    assert!(
        call(&app, "POST", Some(&forged), true)
            .await
            .0
            .is_client_error(),
        "unrelated approval cannot activate"
    );
    assert_eq!(
        catalogue.active.read().await.generation,
        selected.generation
    );
    // Admission settings may shrink without rewriting the policy retained by
    // an existing generation or turning a commit retry into a new publication.
    use brassclaw_product_workflow::MontyVmSettingsStore;
    let settings = runtime.monty_settings_owner.store();
    let before = settings.get("default", "default").await.unwrap();
    let mut smaller = before.execution_limits;
    smaller.max_catalogue_recipes = 1;
    smaller.max_catalogue_components_per_recipe = 1;
    smaller.max_catalogue_variants_per_recipe = 1;
    smaller.max_catalogue_bytes = 128;
    smaller.max_catalogue_actor_bytes = 1;
    let update = serde_json::from_value(json!({
        "expected_revision": before.revision, "execution_limits": smaller,
    }))
    .unwrap();
    let edited = settings
        .upsert("default", "default", &update)
        .await
        .unwrap();
    assert_eq!(catalogue.preparation_policy().await.unwrap(), smaller);
    catalogue.restore_authored().await.unwrap();
    assert_eq!(
        catalogue.active.read().await.generation,
        selected.generation
    );
    assert_eq!(
        call(&app, "POST", Some(&request), true).await.1,
        receipt,
        "idempotent retained receipt ignores newer admission caps"
    );
    let fresh = json!({"activation_id":Uuid::new_v4(),"expected_generation":receipt["generation"],"recipes":request["recipes"]});
    assert!(
        call(&app, "POST", Some(&fresh), true)
            .await
            .0
            .is_client_error()
    );
    assert_eq!(
        catalogue.active.read().await.generation,
        selected.generation
    );
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        effects_before,
        "policy edits and restoration execute no Recipe effects"
    );
    let update = serde_json::from_value(json!({
        "expected_revision": edited.revision, "execution_limits": before.execution_limits,
    }))
    .unwrap();
    settings
        .upsert("default", "default", &update)
        .await
        .unwrap();
    let captured = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    *catalogue.capture_gate.lock().await = Some((captured.clone(), release.clone()));
    let pending_runtime = runtime.clone();
    let pending_chat = chat.clone();
    let pending = tokio::spawn(async move {
        pending_runtime
            .send_user_message(&pending_chat, "authored reply retained across withdrawal")
            .await
            .unwrap()
    });
    tokio::time::timeout(Duration::from_secs(60), captured.notified())
        .await
        .unwrap();
    let expected_catalogue_checksum = hex::encode(<sha2::Sha256 as sha2::Digest>::digest(
        selected.bytes.as_bytes(),
    ));
    let pending_run: Uuid = client
        .query_one(
            "SELECT run_id FROM brassclaw_monty_task_admissions
             WHERE phase='reserved' AND scope->>'thread_id'=$1",
            &[&chat.0.as_str()],
        )
        .await
        .unwrap()
        .get(0);
    let mut wrong_checksum = expected_catalogue_checksum.clone();
    wrong_checksum.replace_range(
        0..1,
        if expected_catalogue_checksum.starts_with('0') {
            "1"
        } else {
            "0"
        },
    );
    let invalid_pin = client
        .execute(
            "UPDATE brassclaw_monty_task_admissions
             SET phase='started',started_at=clock_timestamp(),catalogue_id=$2,catalogue_checksum=$3
             WHERE run_id=$1",
            &[&pending_run, &selected.generation, &wrong_checksum],
        )
        .await
        .unwrap_err();
    assert_eq!(
        invalid_pin.code(),
        Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
    );
    let unchanged = client
        .query_one(
            "SELECT phase,catalogue_id,catalogue_checksum FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&pending_run],
        )
        .await
        .unwrap();
    assert_eq!(unchanged.get::<_, &str>(0), "reserved");
    assert_eq!(unchanged.get::<_, Option<Uuid>>(1), None);
    assert_eq!(unchanged.get::<_, Option<String>>(2), None);
    let withdrawn = json!({"activation_id":Uuid::new_v4(),"expected_generation":receipt["generation"],"recipes":[]});
    let (status, withdrawal) = call(&app, "POST", Some(&withdrawn), true).await;
    assert_eq!(status, StatusCode::OK, "{withdrawal}");
    assert!(catalogue.active.read().await.programs.is_empty());
    assert_eq!(
        selected.programs.len(),
        2,
        "replacement retains the original immutable graph"
    );
    // Retained integrity and eligibility for a fresh capture are different.
    // Verify the actual old graph/approvals in one real database snapshot while
    // the normal task is paused before constructing its task-local bindings.
    {
        let mut integrity_client = rig.pool.get().await.unwrap();
        let tx = integrity_client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        catalogue
            .verify_retained_selection(&tx, &selected)
            .await
            .unwrap();
        assert!(
            catalogue
                .verify_source_selection(&tx, &selected)
                .await
                .is_err()
        );
        tx.commit().await.unwrap();
    }
    release.notify_one();
    let retained_reply = pending.await.unwrap();
    assert_eq!(retained_reply.status, TurnStatus::Completed);
    assert_eq!(
        retained_reply.text.as_deref(),
        Some("retained across withdrawal")
    );
    let retained_admission = client.query_one("SELECT outcome,catalogue_id,catalogue_checksum FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'", &[&retained_reply.run_id.as_uuid()]).await.unwrap();
    let retained_outcome: Value = retained_admission.get(0);
    assert_eq!(retained_reply.run_id.as_uuid(), pending_run);
    assert_eq!(
        retained_admission.get::<_, Option<Uuid>>(1),
        Some(selected.generation)
    );
    assert_eq!(
        retained_admission.get::<_, Option<String>>(2),
        Some(expected_catalogue_checksum)
    );
    let cleared = client
        .execute(
            "UPDATE brassclaw_monty_task_admissions SET catalogue_id=NULL,catalogue_checksum=NULL WHERE run_id=$1",
            &[&pending_run],
        )
        .await
        .unwrap_err();
    assert_eq!(
        cleared.code(),
        Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
    );
    assert!(
        retained_outcome
            .to_string()
            .contains(&selected.generation.to_string()),
        "already captured task keeps its actual normal-match generation"
    );
    catalogue.restore_authored().await.unwrap();
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        effects_before + 2,
        "failed activation and restore perform no Tool effects"
    );
    assert_eq!(
        call(&app, "POST", Some(&request), true).await.1["catalogue_activated"],
        false,
        "an old receipt must not republish a withdrawn generation"
    );
    drop(client);
    drop(app);
    drop(catalogue);
    shutdown_shared_runtime(runtime).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalogue_routing_choices_survive_restart_and_reactivation() {
    let rig = super::super::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let root = tempfile::tempdir().unwrap();
    let input = RebornRuntimeInput::from_services(
        rig.build_input("routing-owner", root.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )
    .with_model_gateway_override(Arc::new(RecordingGateway {
        reply: "unmatched reply".into(),
        requests: Arc::new(StdMutex::new(Vec::new())),
    }))
    .with_poll_settings(PollSettings {
        interval: Duration::from_millis(10),
        max_total: RUNTIME_SEND_TIMEOUT,
    });
    let runtime = Arc::new(Box::pin(build_reborn_runtime(input)).await.unwrap());
    let catalogue = runtime.installed_catalogue.clone();
    let installed = catalogue.reply.program().inputs().instruction();
    assert!(
        !installed.association_approvals().is_empty(),
        "trusted usage must be qualified before this test"
    );
    let new_id = Uuid::new_v4();
    let original = &installed.snapshot().revisions()[&installed.recipe().uuid];
    let mut document = original.draft().document().clone();
    document["name"] = json!("authored-library-reply");
    document["mcp_call"]["name"] = json!("authored_reply");
    document["mcp_call"]["sentence"] = json!("authored reply %");
    for field in ["examples", "negative_examples"] {
        for example in document["mcp_call"][field].as_array_mut().unwrap() {
            *example = json!(
                example
                    .as_str()
                    .unwrap()
                    .replace("publish literal reply", "authored reply")
            );
        }
    }
    for field in ["success_examples", "failure_examples"] {
        for case in document["mcp_qualification"][field].as_array_mut().unwrap() {
            case["command"] = json!(
                case["command"]
                    .as_str()
                    .unwrap()
                    .replace("publish literal reply", "authored reply")
            );
        }
    }
    document["variants"][0]["intent_examples"] = json!(["authored reply %"]);
    let mut alternative = document["variants"][0].clone();
    alternative["variant_key"] = json!("alternative");
    alternative["intent_examples"] = json!(["alternative authored reply %"]);
    document["variants"]
        .as_array_mut()
        .unwrap()
        .push(alternative);
    document["input_layouts"]["alternative"] =
        document["input_layouts"][installed.variant().variant_key.as_str()].clone();
    let draft = ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":new_id,"class_code":21,"document":document,"dependencies":original.draft().dependencies(),"association":null}).to_string()).unwrap();
    let revised = PgComponentRevisionStore::new(rig.pool.clone())
        .stage(&draft, 0)
        .await
        .unwrap();
    let mut refs: Vec<_> = installed
        .snapshot()
        .revisions()
        .values()
        .filter(|r| r.reference().uuid != installed.recipe().uuid)
        .map(|r| reference(r.reference()))
        .collect();
    refs.push(reference(revised));
    let api = crate::build_webui_services(runtime.clone(), None)
        .await
        .unwrap();
    let app = crate::webui_v2_app(
        api,
        crate::WebuiServeConfig::new(
            TenantId::new(runtime.webui_tenant_id()).unwrap(),
            Arc::new(CatalogueOperator),
            Vec::new(),
        ),
    )
    .unwrap();
    let (status, initial) = call(&app, "GET", None, true).await;
    assert_eq!(status, StatusCode::OK);

    let request = json!({"activation_id":Uuid::new_v4(),"expected_generation":initial["generation"],
        "recipes":[{"recipe_id":new_id,"workflow_class":"deterministic","components":refs,"association_approvals":{}}]});
    let (status, activated) = call(&app, "POST", Some(&request), true).await;
    assert_eq!(status, StatusCode::OK, "{activated}");
    let retained = catalogue.active.read().await.clone();
    let vm = runtime.global_monty_owner.client().root_identity();
    let chat = runtime.new_conversation().await.unwrap();
    let captured = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    *catalogue.capture_gate.lock().await = Some((captured.clone(), release.clone()));
    let sending = runtime.clone();
    let conversation = chat.clone();
    let pending = tokio::spawn(async move {
        sending
            .send_user_message(&conversation, "authored reply pinned before disable")
            .await
            .unwrap()
    });
    tokio::time::timeout(Duration::from_secs(60), captured.notified())
        .await
        .unwrap();
    let reply_id = crate::installed_monty_catalogue::id("reply:recipe");
    let choices = json!([reply_id, new_id]);
    let disable = json!({"activation_id":Uuid::new_v4(),"expected_generation":activated["generation"],
        "recipes":request["recipes"],"disabled_recipe_ids":choices});
    let mut invalid = disable.clone();
    invalid["activation_id"] = json!(Uuid::new_v4());
    invalid["disabled_recipe_ids"] = json!([Uuid::new_v4()]);
    assert!(
        call(&app, "POST", Some(&invalid), true)
            .await
            .0
            .is_client_error()
    );
    invalid["disabled_recipe_ids"] = json!([reply_id, reply_id]);
    assert!(
        call(&app, "POST", Some(&invalid), true)
            .await
            .0
            .is_client_error()
    );
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE FUNCTION reject_routing_publication() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'deliberate routing publication rejection'; END $$; CREATE TRIGGER reject_routing_publication BEFORE UPDATE ON brassclaw_monty_active_catalogue FOR EACH ROW EXECUTE FUNCTION reject_routing_publication();").await.unwrap();
    let rejected = call(&app, "POST", Some(&disable), true).await;
    client.batch_execute("DROP TRIGGER reject_routing_publication ON brassclaw_monty_active_catalogue; DROP FUNCTION reject_routing_publication();").await.unwrap();
    assert!(rejected.0.is_server_error(), "{rejected:?}");
    assert_eq!(
        catalogue.active.read().await.generation,
        retained.generation
    );
    assert_eq!(
        client
            .query_one(
                "SELECT catalogue_id FROM brassclaw_monty_active_catalogue WHERE singleton",
                &[]
            )
            .await
            .unwrap()
            .get::<_, Uuid>(0),
        retained.generation
    );
    assert!(!client.query_one("SELECT EXISTS(SELECT 1 FROM brassclaw_monty_catalogue_activations WHERE activation_id=$1)", &[&Uuid::parse_str(disable["activation_id"].as_str().unwrap()).unwrap()]).await.unwrap().get::<_, bool>(0));
    drop(client);
    let (status, disabled) = call(&app, "POST", Some(&disable), true).await;
    assert_eq!(status, StatusCode::OK, "{disabled}");
    release.notify_one();
    let old = pending.await.unwrap();
    assert_eq!(old.status, TurnStatus::Completed);
    assert_eq!(old.text.as_deref(), Some("pinned before disable"));
    assert!(retained.disabled_recipes.is_empty());
    assert_eq!(
        runtime
            .mcp_recipe_discovery()
            .snapshot()
            .unwrap()
            .tools_list(),
        json!({"tools":[]})
    );
    let mut stale = disable.clone();
    stale["activation_id"] = json!(Uuid::new_v4());
    assert_eq!(
        call(&app, "POST", Some(&stale), true).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(call(&app, "POST", Some(&disable), true).await.1, disabled);
    let projection = call(&app, "GET", None, true).await.1;
    assert_eq!(projection["selections"], request["recipes"]);
    assert_eq!(
        projection["disabled_recipe_ids"].as_array().unwrap().len(),
        2
    );
    assert_eq!(projection["packaged_recipe_ids"], json!([reply_id]));
    catalogue.restore_authored().await.unwrap();
    for command in [
        "authored reply disabled custom",
        "publish literal reply disabled builtin",
    ] {
        let result = runtime.send_user_message(&chat, command).await.unwrap();
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(
            result.text.as_deref(),
            Some("unmatched reply"),
            "disabled routing must not disable internal replies/history"
        );
    }
    assert_eq!(runtime.global_monty_owner.client().root_identity(), vm);
    // Withdrawal retains the stable rule. An ordinary catalogue successor that
    // omits routing choices cannot silently re-enable the returning Recipe.
    let withdraw = json!({"activation_id":Uuid::new_v4(),"expected_generation":disabled["generation"],"recipes":[]});
    let (status, withdrawn) = call(&app, "POST", Some(&withdraw), true).await;
    assert_eq!(status, StatusCode::OK, "{withdrawn}");
    assert_eq!(
        call(&app, "POST", Some(&disable), true).await.1["catalogue_activated"],
        false
    );
    let readd = json!({"activation_id":Uuid::new_v4(),"expected_generation":withdrawn["generation"],"recipes":request["recipes"]});
    let (status, readded) = call(&app, "POST", Some(&readd), true).await;
    assert_eq!(status, StatusCode::OK, "{readded}");
    assert_eq!(catalogue.active.read().await.disabled_recipes.len(), 2);
    drop(app);
    drop(catalogue);
    shutdown_shared_runtime(runtime).await.unwrap();
    // Full ordinary startup reseeds the packaged physical anchors. It must
    // reconstruct the persisted routing policy rather than re-enable them.
    let runtime = Arc::new(
        Box::pin(build_reborn_runtime(
            RebornRuntimeInput::from_services(
                rig.build_input("routing-owner", root.path())
                    .with_runtime_policy(local_dev_runtime_policy()),
            )
            .with_model_gateway_override(Arc::new(RecordingGateway {
                reply: "unmatched reply".into(),
                requests: Arc::new(StdMutex::new(Vec::new())),
            }))
            .with_poll_settings(PollSettings {
                interval: Duration::from_millis(10),
                max_total: RUNTIME_SEND_TIMEOUT,
            }),
        ))
        .await
        .unwrap(),
    );
    let owner = runtime.installed_catalogue.clone();
    assert_eq!(
        json!(owner.active.read().await.generation),
        readded["generation"]
    );
    assert_eq!(owner.active.read().await.disabled_recipes.len(), 2);
    assert_eq!(
        runtime
            .mcp_recipe_discovery()
            .snapshot()
            .unwrap()
            .tools_list(),
        json!({"tools":[]})
    );
    let chat = runtime.new_conversation().await.unwrap();
    let blocked = runtime
        .send_user_message(&chat, "authored reply after restart")
        .await
        .unwrap();
    assert_eq!(blocked.text.as_deref(), Some("unmatched reply"));
    let enable = serde_json::from_value(
        json!({"activation_id":Uuid::new_v4(),"expected_generation":readded["generation"],
        "recipes":request["recipes"],"disabled_recipe_ids":[]}),
    )
    .unwrap();
    owner
        .activate_authored("catalogue-operator", enable)
        .await
        .unwrap();
    assert!(owner.active.read().await.disabled_recipes.is_empty());
    let enabled = runtime
        .send_user_message(&chat, "authored reply explicitly enabled")
        .await
        .unwrap();
    assert_eq!(enabled.status, TurnStatus::Completed);
    assert_eq!(enabled.text.as_deref(), Some("explicitly enabled"));
    // Compatibility restoration must reproduce historical bytes, not upgrade
    // their identity by inserting the new empty routing field or current policy.
    let current = owner.active.read().await.clone();
    for (format, omit_later_settings) in [
        ("authored-monty-catalogue/3", true),
        ("authored-monty-catalogue/2", true),
        ("authored-monty-catalogue/2", false),
        ("authored-monty-catalogue/1", false),
    ] {
        let mut document: Value = serde_json::from_str(&current.bytes).unwrap();
        document["format"] = json!(format);
        if !format.ends_with("/3") {
            document
                .as_object_mut()
                .unwrap()
                .remove("disabled_recipe_ids");
        }
        if omit_later_settings {
            // These settings were added after retained preparation policies.
            // Their serde defaults must not insert new fields into old bytes.
            let policy = document["preparation_policy"].as_object_mut().unwrap();
            for key in [
                "default_memory_tree_depth",
                "max_memory_tree_depth",
                "max_coding_read_bytes",
                "max_coding_write_bytes",
                "max_coding_patch_bytes",
                "default_coding_read_lines",
                "max_coding_directory_entries",
                "default_coding_directory_depth",
                "default_coding_max_results",
                "max_coding_output_bytes",
                "default_coding_head_lines",
                "max_coding_scan_bytes",
                "max_coding_visited_entries",
                "max_coding_diff_input_bytes",
                "max_coding_diff_preview_bytes",
                "coding_diff_context_lines",
            ] {
                assert!(policy.remove(key).is_some());
            }
        }
        if format.ends_with("/1") {
            document
                .as_object_mut()
                .unwrap()
                .remove("preparation_policy");
        }
        let bytes = document.to_string();
        use sha2::{Digest, Sha256};
        let checksum = hex::encode(Sha256::digest(bytes.as_bytes()));
        let generation = crate::installed_monty_catalogue::id(&checksum);
        let mut client = rig.pool.get().await.unwrap();
        let tx = client.transaction().await.unwrap();
        tx.execute("INSERT INTO brassclaw_monty_boot_catalogues(catalogue_id,catalogue_bytes,checksum) VALUES($1,$2,$3)", &[&generation,&bytes,&checksum]).await.unwrap();
        tx.execute(
            "UPDATE brassclaw_monty_active_catalogue SET catalogue_id=$1 WHERE singleton",
            &[&generation],
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        drop(client);
        owner.restore_authored().await.unwrap();
        let restored = owner.active.read().await.clone();
        assert_eq!(restored.generation, generation);
        assert_eq!(restored.bytes, bytes);
        assert!(restored.disabled_recipes.is_empty());
        let reply = runtime
            .send_user_message(&chat, "authored reply from historical policy")
            .await
            .unwrap();
        assert_eq!(reply.status, TurnStatus::Completed);
        assert_eq!(reply.text.as_deref(), Some("from historical policy"));
        let retained = rig.pool.get().await.unwrap().query_one(
            "SELECT catalogue_id,catalogue_checksum FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'",
            &[&reply.run_id.as_uuid()],
        ).await.unwrap();
        assert_eq!(retained.get::<_, Option<Uuid>>(0), Some(generation));
        assert_eq!(retained.get::<_, Option<String>>(1), Some(checksum));
    }
    drop(owner);
    shutdown_shared_runtime(runtime).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_catalogue_matching_retains_more_than_eight_thousand_templates() {
    let rig = super::super::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let root = tempfile::tempdir().unwrap();
    let input = RebornRuntimeInput::from_services(
        rig.build_input("eligibility-capacity-owner", root.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )
    .with_model_gateway_override(Arc::new(RecordingGateway {
        reply: "unmatched reply".into(),
        requests: Arc::new(StdMutex::new(Vec::new())),
    }))
    .with_poll_settings(PollSettings {
        interval: Duration::from_millis(10),
        max_total: RUNTIME_SEND_TIMEOUT,
    });
    let runtime = Arc::new(Box::pin(build_reborn_runtime(input)).await.unwrap());
    let catalogue = runtime.installed_catalogue.clone();
    let installed = catalogue.reply.program().inputs().instruction();
    assert!(
        !installed.association_approvals().is_empty(),
        "trusted usage must be qualified before this test"
    );
    let new_id = Uuid::new_v4();
    let original = &installed.snapshot().revisions()[&installed.recipe().uuid];
    let mut document = original.draft().document().clone();
    document["name"] = json!("authored-library-reply");
    document["mcp_call"]["name"] = json!("authored_reply");
    document["mcp_call"]["sentence"] = json!("authored reply %");
    for field in ["examples", "negative_examples"] {
        for example in document["mcp_call"][field].as_array_mut().unwrap() {
            *example = json!(
                example
                    .as_str()
                    .unwrap()
                    .replace("publish literal reply", "authored reply")
            );
        }
    }
    for field in ["success_examples", "failure_examples"] {
        for case in document["mcp_qualification"][field].as_array_mut().unwrap() {
            case["command"] = json!(
                case["command"]
                    .as_str()
                    .unwrap()
                    .replace("publish literal reply", "authored reply")
            );
        }
    }
    document["variants"][0]["intent_examples"] = json!(["authored reply %"]);
    let mut alternative = document["variants"][0].clone();
    alternative["variant_key"] = json!("alternative");
    alternative["intent_examples"] = json!(["alternative authored reply %"]);
    document["variants"]
        .as_array_mut()
        .unwrap()
        .push(alternative);
    document["input_layouts"]["alternative"] =
        document["input_layouts"][installed.variant().variant_key.as_str()].clone();
    let draft = ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":new_id,"class_code":21,"document":document,"dependencies":original.draft().dependencies(),"association":null}).to_string()).unwrap();
    let revised = PgComponentRevisionStore::new(rig.pool.clone())
        .stage(&draft, 0)
        .await
        .unwrap();
    let mut refs: Vec<_> = installed
        .snapshot()
        .revisions()
        .values()
        .filter(|r| r.reference().uuid != installed.recipe().uuid)
        .map(|r| reference(r.reference()))
        .collect();
    refs.push(reference(revised));
    let api = crate::build_webui_services(runtime.clone(), None)
        .await
        .unwrap();
    let app = crate::webui_v2_app(
        api,
        crate::WebuiServeConfig::new(
            TenantId::new(runtime.webui_tenant_id()).unwrap(),
            Arc::new(CatalogueOperator),
            Vec::new(),
        ),
    )
    .unwrap();
    let (status, initial) = call(&app, "GET", None, true).await;
    assert_eq!(status, StatusCode::OK);

    // Lock checksum compatibility with the previous reader, including every
    // input class and a corrupt nonpreferred row. No task/review verdict is made.
    let mut connection = rig.pool.get().await.unwrap();
    let tx = connection
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let scope = &catalogue.qualification_scope;
    let route = &installed.variant().intent_examples[0];
    tx.execute("INSERT INTO reborn_intent_inputs(tenant_id,user_id,agent_id,project_id,input_text,input_class,component_id,component_class_code,source,needs_review,step_link,is_template,template_prefix,template_suffix) SELECT tenant_id,user_id,agent_id,project_id,input_text,1,component_id,component_class_code,source,needs_review,step_link,is_template,template_prefix,template_suffix FROM reborn_intent_inputs WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4 AND component_id=$5 AND input_text=$6 AND input_class=2", &[&scope.tenant_id,&scope.user_id,&scope.agent_id,&scope.project_id,&installed.recipe().uuid,&route]).await.unwrap();
    let mut expected = std::collections::BTreeMap::new();
    // The previous checksum document covered every alias, not only the primary
    // public command. Read its complete small packaged set in this same snapshot
    // as an independent compatibility reference for the new batched reader.
    for template in &installed.variant().intent_examples {
        let (prefix, suffix) =
            brassclaw_engine::memory::template_extractor::parse_template(template).unwrap();
        let rows = tx.query("SELECT input_class FROM reborn_intent_inputs WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4 AND component_id=$5 AND component_class_code=21 AND step_link=$6 AND input_text=$7 AND needs_review=false ORDER BY input_class", &[&scope.tenant_id,&scope.user_id,&scope.agent_id,&scope.project_id,&installed.recipe().uuid,&installed.variant().step_link,&template]).await.unwrap();
        assert!(!rows.is_empty());
        expected.insert(format!("{}:selected:{template}",installed.recipe().uuid),json!({"step_link":installed.variant().step_link,
            "is_template":true,"prefix":prefix,"suffix":suffix,"classes":rows.iter().map(|r|r.get::<_,i16>(0)).collect::<Vec<_>>()}));
    }
    use sha2::{Digest, Sha256};
    let checksum = hex::encode(Sha256::digest(serde_json::to_vec(&expected).unwrap()));
    assert_eq!(
        crate::mcp_command_qualification::verify_routing_metadata(&tx, scope, &[installed])
            .await
            .unwrap(),
        checksum
    );
    tx.execute("UPDATE reborn_intent_inputs SET template_prefix='corrupt' WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4 AND component_id=$5 AND input_text=$6 AND input_class=1", &[&scope.tenant_id,&scope.user_id,&scope.agent_id,&scope.project_id,&installed.recipe().uuid,&route]).await.unwrap();
    assert!(matches!(
        crate::mcp_command_qualification::verify_routing_metadata(&tx, scope, &[installed]).await,
        Err(crate::mcp_recipe_catalogue::McpDiscoveryError::Unqualified)
    ));
    tx.execute("DELETE FROM reborn_intent_inputs WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4 AND component_id=$5", &[&scope.tenant_id,&scope.user_id,&scope.agent_id,&scope.project_id,&installed.recipe().uuid]).await.unwrap();
    assert!(matches!(
        crate::mcp_command_qualification::verify_routing_metadata(&tx, scope, &[installed]).await,
        Err(crate::mcp_recipe_catalogue::McpDiscoveryError::Unqualified)
    ));
    tx.rollback().await.unwrap();
    drop(connection);
    let mut larger = original.draft().document().clone();
    larger["name"] = json!("large-matching-catalogue");
    larger.as_object_mut().unwrap().remove("mcp_call");
    larger.as_object_mut().unwrap().remove("mcp_qualification");
    larger["variants"][0]["intent_examples"] = json!(
        (0..9_000)
            .map(|i| format!("capacity fixture {i} %"))
            .collect::<Vec<_>>()
    );
    let draft = ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":new_id,
        "class_code":21,"document":larger,"dependencies":original.draft().dependencies(),"association":null}).to_string()).unwrap();
    let replacement = PgComponentRevisionStore::new(rig.pool.clone())
        .stage(&draft, 1)
        .await
        .unwrap();
    for selected in &mut refs {
        if selected["uuid"] == json!(new_id) {
            *selected = reference(replacement);
        }
    }
    let request = json!({"activation_id":Uuid::new_v4(),"expected_generation":initial["generation"],
        "recipes":[{"recipe_id":new_id,"workflow_class":"deterministic","components":refs,"association_approvals":{}}]});
    let (status, receipt) = call(&app, "POST", Some(&request), true).await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    let client = rig.pool.get().await.unwrap();
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_intent_inputs WHERE component_id=$1",
                &[&new_id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        9_000
    );
    let chat = runtime.new_conversation().await.unwrap();
    let vm = runtime.global_monty_owner.client().root_identity();
    for index in [0, 4_096, 8_192, 8_999] {
        let expected = format!("exact value {index} ü '{{literal}}'");
        let reply = runtime
            .send_user_message(&chat, &format!("capacity fixture {index} {expected}"))
            .await
            .unwrap();
        assert_eq!(reply.status, TurnStatus::Completed);
        assert_eq!(reply.text.as_deref(), Some(expected.as_str()));
        let outcome: Value = client.query_one("SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'", &[&reply.run_id.as_uuid()]).await.unwrap().get(0);
        assert!(outcome.to_string().contains(&new_id.to_string()));
    }
    assert_eq!(runtime.global_monty_owner.client().root_identity(), vm);
    drop(client);
    drop(app);
    drop(catalogue);
    shutdown_shared_runtime(runtime).await.unwrap();
}

#[path = "component_deployment_tests.rs"]
mod component_deployment_tests;
