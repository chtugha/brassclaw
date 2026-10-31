//! Faults at the actual existing operator activation, native PostgreSQL and
//! source-export boundaries. No substitute publisher, execution or approval.
use super::*;
use crate::installed_monty_catalogue::InstalledMontyCatalogue;
use crate::runtime::RebornRuntime;
use brassclaw_pg::PgPool;

const BARRIER: i64 = 734215980138;
pub(super) async fn reconcile(
    app: &axum::Router,
    id: &str,
    authenticated: bool,
) -> (StatusCode, Value) {
    super::component_deployment_tests::at(
        app,
        "POST",
        &format!("/api/webchat/v2/catalogue/{id}/reconcile"),
        None,
        authenticated,
    )
    .await
}
async fn blocked_backend(
    observer: &tokio_postgres::Client,
    blocker: i32,
    pattern: &str,
) -> Result<i32, tokio::time::error::Elapsed> {
    tokio::time::timeout(RUNTIME_SEND_TIMEOUT, async {
        loop {
            if let Some(row) = observer
                .query_opt(
                    "SELECT pid FROM pg_stat_activity WHERE datname=current_database()
                 AND wait_event_type='Lock' AND $1=ANY(pg_blocking_pids(pid))
                 AND query LIKE $2",
                    &[&blocker, &pattern],
                )
                .await
                .unwrap()
            {
                break row.get(0);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
}
pub(super) async fn reject_before_commit(
    pool: &Arc<PgPool>,
    catalogue: &Arc<InstalledMontyCatalogue>,
    app: &axum::Router,
    request: &Value,
) {
    let observer = pool.get().await.unwrap();
    observer.batch_execute(
        "CREATE FUNCTION brassclaw_test_catalogue_barrier() RETURNS trigger LANGUAGE plpgsql AS $$
           BEGIN PERFORM pg_advisory_xact_lock(734215980138::bigint); RETURN NEW; END $$;
         CREATE TRIGGER brassclaw_test_catalogue_barrier BEFORE UPDATE
           ON brassclaw_monty_active_catalogue FOR EACH ROW
           EXECUTE FUNCTION brassclaw_test_catalogue_barrier();"
    ).await.unwrap();
    let mut owner = pool.get().await.unwrap();
    let lock = owner.transaction().await.unwrap();
    lock.query_one("SELECT pg_advisory_xact_lock($1)", &[&BARRIER])
        .await
        .unwrap();
    let blocker: i32 = lock
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    let original = Uuid::parse_str(request["activation_id"].as_str().unwrap()).unwrap();
    let expected = request["expected_generation"].as_str().unwrap().to_owned();
    let submitted = request.clone();
    let app = app.clone();
    let pending = tokio::spawn(async move { call(&app, "POST", Some(&submitted), true).await });
    let observed = blocked_backend(
        &observer,
        blocker,
        "UPDATE brassclaw_monty_active_catalogue%",
    )
    .await;
    let pid = match observed {
        Ok(pid) => pid,
        Err(error) => {
            pending.abort();
            let _ = pending.await;
            lock.commit().await.unwrap();
            observer.batch_execute("DROP TRIGGER brassclaw_test_catalogue_barrier ON brassclaw_monty_active_catalogue; DROP FUNCTION brassclaw_test_catalogue_barrier()").await.unwrap();
            panic!("actual publication UPDATE must reach the precommit barrier: {error}");
        }
    };
    // Attempt an actual source projection while the uncommitted candidate is
    // held. Killing that publisher must leave this export on the old snapshot.
    let source_owner = catalogue.clone();
    let source_generation = expected.clone();
    let source = tokio::spawn(async move { source_owner.export_sources(&source_generation).await });
    assert!(
        observer
            .query_one("SELECT pg_terminate_backend($1)", &[&pid])
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    assert_eq!(pending.await.unwrap().0, StatusCode::SERVICE_UNAVAILABLE);
    lock.commit().await.unwrap();
    observer.batch_execute("DROP TRIGGER brassclaw_test_catalogue_barrier ON brassclaw_monty_active_catalogue; DROP FUNCTION brassclaw_test_catalogue_barrier()").await.unwrap();
    let exported = source.await.unwrap().unwrap();
    assert_eq!(exported.generation, expected);
    let row = observer
        .query_one(
            "SELECT a.catalogue_id,(SELECT count(*) FROM brassclaw_monty_catalogue_activations
           WHERE activation_id=$1) FROM brassclaw_monty_active_catalogue a WHERE singleton",
            &[&original],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, Uuid>(0).to_string(), expected);
    assert_eq!(
        row.get::<_, i64>(1),
        0,
        "activation and invalidation must roll back together"
    );
}
pub(super) async fn assert_committed(
    pool: &Arc<PgPool>,
    app: &axum::Router,
    request: &Value,
    receipt: &Value,
) {
    let id = Uuid::parse_str(request["activation_id"].as_str().unwrap()).unwrap();
    let client = pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT a.catalogue_id,a.activation_id,p.prefix_invalidation,c.checksum
         FROM brassclaw_monty_active_catalogue a
         JOIN brassclaw_monty_catalogue_activations p ON p.activation_id=a.activation_id
         JOIN brassclaw_monty_boot_catalogues c ON c.catalogue_id=a.catalogue_id
         WHERE a.singleton",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(json!(row.get::<_, Uuid>(0)), receipt["generation"]);
    assert_eq!(row.get::<_, Uuid>(1), id);
    let invalidation: Value = row.get(2);
    assert_eq!(invalidation["catalogue_generation"], receipt["generation"]);
    assert_eq!(invalidation["catalogue_checksum"], row.get::<_, String>(3));
    assert!(
        !invalidation["source_documents"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let profiles = client
        .query(
            "SELECT name,revision FROM reborn_prefix_profiles ORDER BY name COLLATE \"C\"",
            &[],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|r| json!({"name":r.get::<_, String>(0),"revision":r.get::<_, i64>(1)}))
        .collect::<Vec<_>>();
    assert_eq!(invalidation["profiles"], json!(profiles));
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_catalogue_activations WHERE activation_id=$1",
                &[&id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    let (status, current) = call(app, "GET", None, true).await;
    assert_eq!(status, StatusCode::OK, "{current}");
    assert_eq!(
        current["publication"]["desired_generation"],
        receipt["generation"]
    );
    assert_eq!(
        current["publication"]["effective_generation"],
        receipt["generation"]
    );
    assert_eq!(
        current["publication"]["discovery_effective_generation"],
        receipt["generation"]
    );
    assert_eq!(
        current["publication"]["prefix_invalidation_id"],
        request["activation_id"]
    );
    assert_eq!(current["publication"]["pending"], false);
    assert_eq!(current["publication"]["inline_prefix_ready"], true);
    assert_eq!(current["publication"]["prefix_rebuild_pending"], true);
}
pub(super) async fn reject_discovery_after_commit(
    pool: &Arc<PgPool>,
    app: &axum::Router,
    request: &Value,
) -> Value {
    let observer = pool.get().await.unwrap();
    let mut owner = pool.get().await.unwrap();
    let lock = owner.transaction().await.unwrap();
    lock.batch_execute("LOCK TABLE brassclaw_monty_task_admissions IN ACCESS EXCLUSIVE MODE")
        .await
        .unwrap();
    let blocker: i32 = lock
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    let submitted = request.clone();
    let owned_app = app.clone();
    let pending =
        tokio::spawn(async move { call(&owned_app, "POST", Some(&submitted), true).await });
    let observed = blocked_backend(
        &observer,
        blocker,
        "%FROM brassclaw_monty_tool_invocations i JOIN brassclaw_monty_task_admissions a%",
    )
    .await;
    let pid = match observed {
        Ok(pid) => pid,
        Err(error) => {
            pending.abort();
            let _ = pending.await;
            lock.commit().await.unwrap();
            panic!("actual postcommit discovery must reach the blocked observation: {error}");
        }
    };
    let id = Uuid::parse_str(request["activation_id"].as_str().unwrap()).unwrap();
    let committed = observer
        .query_one(
            "SELECT catalogue_id,prefix_invalidation FROM brassclaw_monty_catalogue_activations
         WHERE activation_id=$1",
            &[&id],
        )
        .await
        .unwrap();
    let generation = committed.get::<_, Uuid>(0);
    assert_eq!(
        committed.get::<_, Value>(1)["catalogue_generation"],
        json!(generation)
    );
    assert!(
        observer
            .query_one("SELECT pg_terminate_backend($1)", &[&pid])
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    assert_eq!(pending.await.unwrap().0, StatusCode::SERVICE_UNAVAILABLE);
    // Keep the real discovery dependency blocked while reading status. A status
    // projection must not wait behind activation or command qualification.
    let (status, state) = tokio::time::timeout(RUNTIME_SEND_TIMEOUT, call(app, "GET", None, true))
        .await
        .expect("publication pending status remains readable");
    assert_eq!(status, StatusCode::OK, "{state}");
    assert_eq!(
        state["publication"]["desired_generation"],
        json!(generation)
    );
    assert_eq!(
        state["publication"]["effective_generation"],
        json!(generation)
    );
    assert_eq!(state["publication"]["pending"], true);
    assert_eq!(
        state["publication"]["discovery_effective_generation"],
        Value::Null
    );
    assert_eq!(state["publication"]["inline_prefix_ready"], true);
    assert_eq!(state["publication"]["prefix_rebuild_pending"], true);
    lock.commit().await.unwrap();
    drop(owner);
    drop(observer);
    assert_eq!(
        reconcile(app, request["activation_id"].as_str().unwrap(), false)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        reconcile(app, &Uuid::new_v4().to_string(), true).await.0,
        StatusCode::NOT_FOUND
    );
    let (status, receipt) = reconcile(app, request["activation_id"].as_str().unwrap(), true).await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["generation"], json!(generation));
    assert_eq!(
        call(app, "POST", Some(request), true).await.1,
        receipt,
        "reference reconciliation and original exact retry read the same activation"
    );
    assert_committed(pool, app, request, &receipt).await;
    receipt
}

pub(super) async fn failed_build_keeps_invalidation(
    pool: &Arc<PgPool>,
    app: &axum::Router,
    runtime: &Arc<RebornRuntime>,
    request: &Value,
    receipt: &Value,
) {
    use brassclaw_product_workflow::instance_tools::{
        InstanceToolSettings, UpdateInstanceToolSetting,
    };
    let catalogue = &runtime.installed_catalogue;
    let program = catalogue._compiler_program.as_ref().unwrap();
    let brassclaw_engine::executor::retained_recipe::RetainedProgram::Tools(tools) =
        program.program()
    else {
        panic!("qualified source compiler must retain native bindings")
    };
    let json_tool = tools
        .bindings()
        .values()
        .find(|b| b.capability_id() == "builtin.json")
        .unwrap()
        .tool()
        .uuid;
    let policies = crate::instance_tool_settings::PgInstanceToolSettings::new(pool.clone());
    let original = policies
        .list()
        .await
        .unwrap()
        .into_iter()
        .find(|p| p.tool_id == json_tool)
        .unwrap();
    assert!(original.enabled);
    let blocked = policies
        .update(
            json_tool,
            &UpdateInstanceToolSetting {
                expected_revision: original.revision,
                enabled: false,
                limits: original.limits.clone(),
            },
        )
        .await
        .unwrap();
    let revision = pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT revision FROM reborn_prefix_profiles WHERE name='base-prompt'",
            &[],
        )
        .await
        .unwrap()
        .get::<_, i64>(0) as u64;
    let id = Uuid::new_v4();
    let caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        runtime.thread_scope.tenant_id.clone(),
        runtime.actor_user_id.clone(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
    );
    assert_eq!(
        runtime
            .submit_current_core_prefix_snapshot(id, "base-prompt", revision, caller)
            .await
            .unwrap(),
        id
    );
    let definition = crate::prefix_job_definitions::Store::new(pool.clone())
        .get(id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        runtime
            .wait_for_terminal(
                &definition.scope,
                TurnRunId::from_uuid(id),
                &tokio_util::sync::CancellationToken::new()
            )
            .await
            .unwrap(),
        TurnStatus::Failed
    );
    let row = pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT phase,outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, &str>(0), "settled");
    assert_eq!(row.get::<_, Value>(1)["status"], "failed");
    assert!(row.get::<_, Value>(1).get("internal_receipt").is_none());
    // Unrelated approved provider-free work is not globally paused by a failed
    // prefix build or pending generated-prefix publication.
    let chat = runtime.new_conversation().await.unwrap();
    let answer = runtime
        .send_user_message(
            &chat,
            "authored reply independent while prefix build is blocked",
        )
        .await
        .unwrap();
    assert_eq!(answer.status, TurnStatus::Completed);
    assert_eq!(
        answer.text.as_deref(),
        Some("independent while prefix build is blocked")
    );
    policies
        .update(
            json_tool,
            &UpdateInstanceToolSetting {
                expected_revision: blocked.revision,
                enabled: original.enabled,
                limits: original.limits,
            },
        )
        .await
        .unwrap();
    assert_committed(pool, app, request, receipt).await;
}

/// Database migration only: legal historical row fixtures are not approved
/// workflows or runtime restoration evidence. All migrations are actual files.
pub(super) async fn upgrade_publication_records(client: &mut tokio_postgres::Client) {
    use sha2::{Digest, Sha256};
    brassclaw_pg::migrations::migrations::runner()
        .set_target(refinery::Target::Version(176))
        .run_async(client)
        .await
        .unwrap();
    let prefix = hex::encode(Sha256::digest(b"historical inline fixture"));
    let bytes = json!({"format":"legacy-migration-fixture/1","prefix_checksum":prefix}).to_string();
    let checksum = hex::encode(Sha256::digest(bytes.as_bytes()));
    let generation = crate::installed_monty_catalogue::id(&checksum);
    let activation = Uuid::new_v4();
    let request = json!({"activation_id":activation,"expected_generation":generation,"recipes":[]})
        .to_string();
    let request_checksum = hex::encode(Sha256::digest(request.as_bytes()));
    client.execute("INSERT INTO brassclaw_monty_boot_catalogues(catalogue_id,catalogue_bytes,checksum) VALUES($1,$2,$3)", &[&generation,&bytes,&checksum]).await.unwrap();
    client.execute("INSERT INTO brassclaw_monty_catalogue_activations(activation_id,actor,request_bytes,request_checksum,catalogue_id) VALUES($1,'legacy-operator',$2,$3,$4)", &[&activation,&request,&request_checksum,&generation]).await.unwrap();
    client
        .execute(
            "INSERT INTO brassclaw_monty_active_catalogue(singleton,catalogue_id) VALUES(true,$1)",
            &[&generation],
        )
        .await
        .unwrap();
    let original = client.query_one("SELECT encode(sha256(convert_to(to_jsonb(a)::text,'UTF8')),'hex') FROM brassclaw_monty_catalogue_activations a WHERE activation_id=$1", &[&activation]).await.unwrap().get::<_, String>(0);
    let pointer = client.query_one("SELECT encode(sha256(convert_to(to_jsonb(a)::text,'UTF8')),'hex') FROM brassclaw_monty_active_catalogue a WHERE singleton", &[]).await.unwrap().get::<_, String>(0);
    brassclaw_pg::migrations::migrations::runner()
        .set_target(refinery::Target::Version(177))
        .run_async(client)
        .await
        .unwrap();
    let retained = client.query_one("SELECT prefix_invalidation,encode(sha256(convert_to((to_jsonb(a)-'prefix_invalidation')::text,'UTF8')),'hex') FROM brassclaw_monty_catalogue_activations a WHERE activation_id=$1", &[&activation]).await.unwrap();
    assert_eq!(retained.get::<_, Option<Value>>(0), None);
    assert_eq!(retained.get::<_, String>(1), original);
    let retained = client.query_one("SELECT activation_id,encode(sha256(convert_to((to_jsonb(a)-'activation_id')::text,'UTF8')),'hex') FROM brassclaw_monty_active_catalogue a WHERE singleton", &[]).await.unwrap();
    assert_eq!(retained.get::<_, Option<Uuid>>(0), None);
    assert_eq!(retained.get::<_, String>(1), pointer);
    let missing = client.execute("INSERT INTO brassclaw_monty_catalogue_activations(activation_id,actor,request_bytes,request_checksum,catalogue_id) VALUES($1,'legacy-operator',$2,$3,$4)", &[&Uuid::new_v4(),&request,&request_checksum,&generation]).await.unwrap_err();
    assert_eq!(
        missing.code(),
        Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
    );
    let backfill = client.execute("UPDATE brassclaw_monty_catalogue_activations SET prefix_invalidation='{}'::jsonb WHERE activation_id=$1", &[&activation]).await.unwrap_err();
    assert_eq!(
        backfill.code(),
        Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
    );
    let other =
        json!({"format":"legacy-migration-fixture/1","prefix_checksum":prefix,"different":true})
            .to_string();
    let other_checksum = hex::encode(Sha256::digest(other.as_bytes()));
    let other_generation = crate::installed_monty_catalogue::id(&other_checksum);
    client.execute("INSERT INTO brassclaw_monty_boot_catalogues(catalogue_id,catalogue_bytes,checksum) VALUES($1,$2,$3)", &[&other_generation,&other,&other_checksum]).await.unwrap();
    let mismatched = client.execute("UPDATE brassclaw_monty_active_catalogue SET catalogue_id=$1,activation_id=$2 WHERE singleton", &[&other_generation,&activation]).await.unwrap_err();
    assert_eq!(
        mismatched.code(),
        Some(&tokio_postgres::error::SqlState::FOREIGN_KEY_VIOLATION)
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
        generation
    );
}

pub(super) async fn one_connection_reconciliation(
    pool: &Arc<PgPool>,
    catalogue: &Arc<InstalledMontyCatalogue>,
    app: &axum::Router,
    activation: &str,
) {
    let original_capacity = pool.status().max_size;
    pool.resize(1);
    let observed = tokio::time::timeout(RUNTIME_SEND_TIMEOUT, async {
        tokio::join!(
            catalogue.restore_authored(),
            reconcile(app, activation, true),
            catalogue.refresh_active_discovery()
        )
    })
    .await;
    // Restore capacity even if an actual lock-order regression times out.
    pool.resize(original_capacity);
    let (restored, (status, receipt), discovery) =
        observed.expect("one-connection publication/discovery reconciliation must not deadlock");
    restored.unwrap();
    discovery.unwrap();
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["activation_id"], activation);
    assert_eq!(receipt["catalogue_activated"], true);
}

/// Reuse the existing request-recording gateway to observe the actual global
/// Monty/model/reply ports. This checks context selection, not real inference.
pub(super) async fn assert_new_inline_model_context(
    runtime: &Arc<RebornRuntime>,
    pool: &Arc<PgPool>,
    requests: &Arc<StdMutex<Vec<HostManagedModelRequest>>>,
    old: &crate::authored_monty_catalogue::CatalogueGeneration,
) {
    use sha2::{Digest, Sha256};
    let selected = runtime.installed_catalogue.active.read().await.clone();
    let chat = runtime.new_conversation().await.unwrap();
    let answer = runtime
        .send_user_message(&chat, "catalogue model context coherence probe")
        .await
        .unwrap();
    assert_eq!(answer.status, TurnStatus::Completed);
    assert_eq!(answer.text.as_deref(), Some("unmatched reply"));
    {
        let recorded = requests.lock().unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].run_id, answer.run_id);
        assert!(
            recorded[0]
                .messages
                .iter()
                .any(|m| m.role == HostManagedModelMessageRole::System
                    && m.content == selected.prefix)
        );
        assert!(
            !recorded[0]
                .messages
                .iter()
                .any(|m| m.role == HostManagedModelMessageRole::System && m.content == old.prefix)
        );
    }
    // Ordinary No-Match uses the global host's model port, not a selected
    // host.kohai_complete Tool invocation. Correlate its actual request above
    // with the ordinary admission's immutable catalogue and prefix checksum.
    let retained = pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT a.catalogue_id,a.catalogue_checksum,c.checksum,
                    c.catalogue_bytes::jsonb->>'prefix_checksum',a.outcome
             FROM brassclaw_monty_task_admissions a
             JOIN brassclaw_monty_boot_catalogues c ON c.catalogue_id=a.catalogue_id
             WHERE a.run_id=$1 AND a.phase='settled' AND a.catalogue_reference_version=1",
            &[&answer.run_id.as_uuid()],
        )
        .await
        .unwrap();
    assert_eq!(retained.get::<_, Uuid>(0), selected.generation);
    let catalogue_checksum = hex::encode(Sha256::digest(selected.bytes.as_bytes()));
    assert_eq!(retained.get::<_, String>(1), catalogue_checksum);
    assert_eq!(retained.get::<_, String>(2), catalogue_checksum);
    assert_eq!(
        retained.get::<_, String>(3),
        hex::encode(Sha256::digest(selected.prefix.as_bytes()))
    );
    let outcome: Value = retained.get(4);
    assert_eq!(outcome["status"], "completed");
    assert_eq!(outcome["execution"]["intent_outcome"], "no_match");
    assert_eq!(outcome["execution"]["model_exchange_count"], 1);
}
