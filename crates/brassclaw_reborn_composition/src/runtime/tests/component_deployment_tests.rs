//! Retained review submissions -> read-only preparation -> actual operator
//! publication -> ordinary global Monty. Reuses actual bundled usage approval.
#[path = "prefix_generation_history_http_tests.rs"]
mod history_http_tests;
#[path = "prefix_pair_construction_tests.rs"]
mod pair_construction_tests;
#[path = "prefix_model_setup_http_tests.rs"]
mod setup_http_tests;
use super::super::model_encoding_tests::oracle_python;
use super::*;
use crate::runtime::RebornRuntime;
use secrecy::ExposeSecret;
pub(super) async fn at(
    app: &axum::Router,
    method: &str,
    path: &str,
    body: Option<&Value>,
    authenticated: bool,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_component_deployment_preserves_recipes_and_pins_review_subjects() {
    let rig = super::super::super::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let root = tempfile::tempdir().unwrap();
    let snapshot_request = Uuid::new_v4();
    use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![
                snapshot_request.as_bytes().to_vec(),
                b"snapshot_bytes".to_vec(),
            ],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    // All application readers share the actual wire-transport pool. A second
    // 16-connection metadata pool can exceed embedded PostgreSQL's 20 total
    // connections when key-presence checks run concurrently.
    let pool = transport.pool.clone();
    let input = RebornRuntimeInput::from_services(
        crate::RebornBuildInput::postgres_with_reborn_home(
            "component-deployment-owner",
            (*transport.pool).clone(),
            brassclaw_secrets::SecretMaterial::from(transport.connection_string().to_owned()),
            root.path().to_path_buf(),
        )
        .with_runtime_policy(local_dev_runtime_policy()),
    )
    .with_model_gateway_override(Arc::new(RecordingGateway {
        reply: "unmatched reply".into(),
        requests: Arc::new(StdMutex::new(Vec::new())),
    }))
    .with_poll_settings(PollSettings {
        interval: Duration::from_millis(10),
        // This caller observes a complete ten-MiB source job on CI too.
        // Use the existing default task budget for its outer test observation;
        // the old 120-second observer could expire while legitimate work ran.
        // This changes neither production polling nor the VM compute account.
        max_total: Duration::from_secs(
            brassclaw_product_workflow::default_monty_vm_settings().max_duration_secs,
        ),
    });
    let runtime = Arc::new(Box::pin(build_reborn_runtime(input)).await.unwrap());
    let catalogue = runtime.installed_catalogue.clone();
    crate::bootstrap_encoding_approval::assert_retained_proof_checks_actual_journal(
        catalogue.pool.clone(),
        &catalogue.encoding_approvals,
    )
    .await;
    crate::bootstrap_measurement_approval::assert_retained_proof_checks_journal(
        catalogue.pool.clone(),
        catalogue
            .measurement_qualification
            .as_ref()
            .expect("closed measurement qualification before readiness"),
    )
    .await;
    assert!(
        brassclaw_loop_support::SystemBundleSource::get_system_bundle_for_run(
            catalogue.as_ref(),
            "component-deployment-owner",
            "bootstrap",
            &Uuid::new_v4().to_string()
        )
        .await
        .is_err(),
        "a missing retained capture must never substitute the active prefix"
    );

    let source_candidate = &catalogue._source_compiler_candidates.source;
    assert_eq!(
        source_candidate
            .program()
            .inputs()
            .instruction()
            .snapshot()
            .roots()
            .len(),
        1
    );
    let digest_candidate = &catalogue._source_compiler_candidates.digest;
    let command = digest_candidate
        .program()
        .inputs()
        .bind_variant_example(
            "digest utf8 json=%",
            "digest utf8 json=[\"a\",\"b\"]",
            &json!({}),
        )
        .unwrap();
    assert_eq!(command, json!({"texts_json":"[\"a\",\"b\"]"}));
    assert_eq!(
        source_candidate
            .program()
            .inputs()
            .instruction()
            .recipe()
            .uuid,
        crate::installed_monty_catalogue::id("prefix-source-compiler:recipe")
    );
    assert!(
        source_candidate
            .program()
            .inputs()
            .instruction()
            .association_approvals()
            .is_empty(),
        "retaining/inspecting a compiler candidate must not fabricate qualification"
    );
    let brassclaw_engine::executor::retained_recipe::RetainedProgram::Tools(source_program) =
        source_candidate.program()
    else {
        panic!("compiler Tool program required")
    };
    assert_eq!(source_program.bindings().len(), 5);
    assert_eq!(
        source_program
            .bindings()
            .values()
            .filter(|binding| binding.capability_id() == "builtin.json")
            .count(),
        4
    );
    assert_eq!(
        catalogue.generation_approvals.len(),
        2,
        "installation must commit actual storage-usage proofs before readiness"
    );
    let client = pool.get().await.unwrap();
    let dispatches: i64 = client
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get(0);
    drop(client);
    let reused = Box::pin(crate::bootstrap_generation_approval::qualify(
        pool.clone(),
        catalogue.kernel.clone(),
        &catalogue._generation_candidates,
        &catalogue.worker,
        catalogue.startup_executor.clone(),
    ))
    .await
    .unwrap();
    assert_eq!(reused.len(), 2);
    let compiler_approval = catalogue
        .compiler_approval
        .as_ref()
        .expect("closed compiler qualification before readiness");
    let reused_compiler = Box::pin(crate::bootstrap_compiler_approval::qualify(
        pool.clone(),
        catalogue.kernel.clone(),
        &catalogue._source_compiler_candidates.source,
        &catalogue.generation_approvals,
        &catalogue.worker,
        catalogue.startup_executor.clone(),
    ))
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        reused_compiler.approval_id(),
        compiler_approval.approval_id()
    );
    crate::bootstrap_compiler_approval::assert_incomplete_intent_is_not_replayed(
        pool.clone(),
        compiler_approval,
    )
    .await;
    let private = catalogue._compiler_program.as_ref().unwrap();
    assert_eq!(
        private
            .program()
            .inputs()
            .instruction()
            .association_approvals()
            .len(),
        5
    );
    let candidate_instruction = catalogue
        ._source_compiler_candidates
        .source
        .program()
        .inputs()
        .instruction();
    let private_route:brassclaw_product_workflow::CatalogueRecipeSelection = serde_json::from_value(json!({
        "recipe_id":candidate_instruction.recipe().uuid,"workflow_class":"deterministic",
        "components":candidate_instruction.snapshot().revisions().values().map(|revision| reference(revision.reference())).collect::<Vec<_>>(),"association_approvals":{},
    })).unwrap();
    let mut route_client = pool.get().await.unwrap();
    let route_tx = route_client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    assert!(
        catalogue
            .prepare_authored(
                &route_tx,
                &[private_route],
                catalogue.preparation_policy().await.unwrap()
            )
            .await
            .is_err(),
        "an internal receipt workflow must not acquire a public chat route from its private qualification"
    );
    route_tx.rollback().await.unwrap();
    drop(route_client);
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    for approval in &reused {
        approval.verify_in_transaction(&tx).await.unwrap();
    }
    assert_eq!(
        tx.query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        dispatches,
        "committed qualification must not replay behavioral effects"
    );
    tx.commit().await.unwrap();
    drop(client);

    crate::prefix_reference_seeds::assert_database_sources(&catalogue).await;
    crate::prefix_job_definitions::assert_metadata_only(&catalogue).await;
    let (prefix_job_id, sempai_job_id) =
        crate::runtime::prefix_job_submission::assert_global_snapshot_submission(
            &runtime,
            snapshot_request,
            &transport,
        )
        .await;

    // The canonical Retain workflow may use only its own qualified formatter.
    // A different Recipe sharing the storage usage cannot borrow that coverage.
    for candidate in &catalogue._generation_candidates {
        let instruction = candidate.program().inputs().instruction();
        let selection: brassclaw_product_workflow::CatalogueRecipeSelection =
            serde_json::from_value(json!({"recipe_id":instruction.recipe().uuid,
                "workflow_class":"deterministic","components":instruction.snapshot().revisions().values()
                    .map(|r| reference(r.reference())).collect::<Vec<_>>(),"association_approvals":{}})).unwrap();
        let mut client = pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        let prepared = catalogue
            .prepare_authored(
                &tx,
                &[selection],
                catalogue.preparation_policy().await.unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(prepared.programs.len(), 1);
        tx.rollback().await.unwrap();
        let original = &instruction.snapshot().revisions()[&instruction.recipe().uuid];
        if original.draft().document()["name"] != "prefix-generation-retain" {
            continue;
        }
        let changed_id = Uuid::new_v4();
        let changed = ComponentRevisionDraft::from_json(
            &json!({"format":"component-revision/1",
            "uuid":changed_id,"class_code":21,"document":original.draft().document(),
            "dependencies":original.draft().dependencies(),"association":null})
            .to_string(),
        )
        .unwrap();
        let changed_ref =
            brassclaw_skills::revision_store::PgComponentRevisionStore::new(pool.clone())
                .stage(&changed, 0)
                .await
                .unwrap();
        let mut components: Vec<_> = instruction
            .snapshot()
            .revisions()
            .values()
            .filter(|r| r.reference().uuid != instruction.recipe().uuid)
            .map(|r| reference(r.reference()))
            .collect();
        components.push(reference(changed_ref));
        let selection = serde_json::from_value(
            json!({"recipe_id":changed_id,"workflow_class":"deterministic",
            "components":components,"association_approvals":{}}),
        )
        .unwrap();
        let mut client = pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        assert!(
            catalogue
                .prepare_authored(
                    &tx,
                    &[selection],
                    catalogue.preparation_policy().await.unwrap()
                )
                .await
                .is_err(),
            "a different Recipe cannot borrow packaged pure-component qualification"
        );
        tx.rollback().await.unwrap();
    }

    let retained_storage = catalogue
        .kernel
        .builtin(
            &brassclaw_host_api::CapabilityId::new(
                brassclaw_host_runtime::PREFIX_GENERATION_CAPABILITY_ID,
            )
            .unwrap(),
            None,
        )
        .unwrap();
    assert!(
        retained_storage.native_identity().is_some(),
        "the registered primitive must retain its actual linked image/schema"
    );
    retained_storage.verify_native_artifact().await.unwrap();
    crate::prefix_generation_behavior_tests::assert_native_generation_usages(&catalogue).await;
    crate::prefix_generation_behavior_tests::assert_native_source_compiler(&catalogue).await;
    let source_behavior = crate::bootstrap_compiler_validation::execute(
        catalogue.startup_executor.clone(),
        catalogue.pool.clone(),
        catalogue.kernel.clone(),
        catalogue._source_compiler_candidates.source.clone(),
        &catalogue.worker,
        "development-prefix-compiler/1",
    )
    .await
    .unwrap();
    assert_eq!(
        source_behavior["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|record| record["case"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "base-literal",
            "base-deduplicated",
            "maker-literal",
            "maker-deduplicated",
            "base-trailing-inventory",
            "maker-trailing-inventory",
            "base-stable-sharing",
            "maker-stable-sharing",
            "base-contextual",
            "maker-contextual",
            "sempai-contextual",
            "duplicate",
            "missing-guide",
            "denied",
            "invalid-revision"
        ]
    );
    let mut proof_client = catalogue.pool.get().await.unwrap();
    let proof_tx = proof_client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    crate::bootstrap_compiler_validation::verify(
        &proof_tx,
        &catalogue._source_compiler_candidates.source,
        &source_behavior,
        "development-prefix-compiler/1",
    )
    .await
    .unwrap();
    crate::bootstrap_compiler_validation::verify(
        &proof_tx,
        &catalogue._source_compiler_candidates.source,
        &source_behavior,
        "development-prefix-compiler/1",
    )
    .await
    .unwrap();
    assert!(
        crate::bootstrap_compiler_validation::verify(
            &proof_tx,
            &catalogue._source_compiler_candidates.source,
            &source_behavior,
            "wrong-qualification-scope",
        )
        .await
        .is_err()
    );
    let mut forged = source_behavior.clone();
    forged["cases"][0]["execution"]["semantic_approval"] = json!(true);
    assert!(
        crate::bootstrap_compiler_validation::verify(
            &proof_tx,
            &catalogue._source_compiler_candidates.source,
            &forged,
            "development-prefix-compiler/1",
        )
        .await
        .is_err()
    );
    let mut forged = source_behavior.clone();
    let calls = forged["cases"][0]["invocations"].as_object_mut().unwrap();
    calls.values_mut().next().unwrap()["arguments"] = json!("0".repeat(64));
    assert!(
        crate::bootstrap_compiler_validation::verify(
            &proof_tx,
            &catalogue._source_compiler_candidates.source,
            &forged,
            "development-prefix-compiler/1",
        )
        .await
        .is_err()
    );
    proof_tx.commit().await.unwrap();
    drop(proof_client);

    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let large = oracle_python().arg("-c")
        .arg("import json,sys; from test_prefix_build_worker import large_source_request; import prefix_build_worker; sys.stdout.buffer.write(prefix_build_worker.build(json.dumps(large_source_request()).encode()))")
        .env("PYTHONPATH",checkout.join("scripts/prefix"))
        .env("HF_HUB_OFFLINE","1").output().unwrap();
    assert!(
        large.status.success(),
        "{}",
        String::from_utf8_lossy(&large.stderr)
    );
    let large: Value = serde_json::from_slice(&large.stdout).unwrap();
    crate::prefix_generation_behavior_tests::assert_native_large_generation(
        &catalogue,
        large["generation_bytes"].as_str().unwrap().to_owned(),
    )
    .await;

    let installed = catalogue.reply.program().inputs().instruction();
    assert!(
        !installed.association_approvals().is_empty(),
        "trusted usage must be qualified before this test"
    );

    let original = &installed.snapshot().revisions()[&installed.recipe().uuid];
    let dependencies: Vec<_> = installed
        .snapshot()
        .revisions()
        .values()
        .filter(|r| r.reference().uuid != installed.recipe().uuid)
        .map(|r| reference(r.reference()))
        .collect();
    let make_recipe = |uuid: Uuid, label: &str| {
        let mut document = original.draft().document().clone();
        document["name"] = json!(format!("deployment-{label}"));
        document["mcp_call"]["name"] = json!(format!("deployment_{label}"));
        let document: Value = serde_json::from_str(&document.to_string().replace(
            "publish literal reply",
            &format!("deployment {label} reply"),
        ))
        .unwrap();
        ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":uuid,"class_code":21,"document":document,"dependencies":original.draft().dependencies(),"association":null}).to_string()).unwrap()
    };
    let api = crate::build_webui_services(runtime.clone(), None)
        .await
        .unwrap();
    let mut model_bundle = api.clone();
    model_bundle.api = Arc::new(
        brassclaw_product_workflow::RebornServices::new(
            runtime.webui_thread_service(),
            runtime.webui_turn_coordinator(),
        )
        .with_llm_config_service(
            prefix_metadata_llm_service(pool.clone(), runtime.webui_tenant_id()).await,
        )
        .with_interceptor_config_service(Arc::new(
            crate::interceptor_config_service::RebornInterceptorConfigService::new(
                pool.clone(),
                runtime.webui_tenant_id(),
            )
            .with_runtime(runtime.clone()),
        )),
    );
    let model_app = crate::webui_v2_app(
        model_bundle,
        crate::WebuiServeConfig::new(
            TenantId::new(runtime.webui_tenant_id()).unwrap(),
            Arc::new(CatalogueOperator),
            Vec::new(),
        ),
    )
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
    let job_path = format!("/api/webchat/v2/prefixes/base-prompt/jobs/{prefix_job_id}");
    assert_eq!(
        at(&app, "GET", &job_path, None, false).await.0,
        StatusCode::UNAUTHORIZED
    );
    for _ in 0..2 {
        let (status, job) = at(&app, "GET", &job_path, None, true).await;
        assert_eq!(status, StatusCode::OK, "{job}");
        assert_eq!(job["job_id"], prefix_job_id.to_string());
        assert_eq!(job["run_status"], "Completed");
        assert!(!job["thread_id"].as_str().unwrap().is_empty());
        assert_eq!(job["admission_phase"], "settled");
        assert_eq!(job["answered_tool_calls"], 5);
        assert_eq!(job["pending_tool_calls"], 0);
        assert!(
            job["internal_receipt"]
                .as_str()
                .unwrap()
                .starts_with("prefix-generation:base-prompt:")
        );
        assert!(
            job["result_ref"]
                .as_str()
                .unwrap()
                .starts_with("result:monty.")
        );
        assert!(job.get("input").is_none() && job.get("program_bytes").is_none());
    }
    assert_eq!(
        at(
            &app,
            "GET",
            &format!("/api/webchat/v2/prefixes/prefix-maker/jobs/{prefix_job_id}"),
            None,
            true
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        at(
            &app,
            "GET",
            &format!(
                "/api/webchat/v2/prefixes/base-prompt/jobs/{}",
                Uuid::new_v4()
            ),
            None,
            true
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        at(
            &app,
            "GET",
            "/api/webchat/v2/prefixes/base-prompt/jobs/not-a-uuid",
            None,
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        pool.get()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
                &[&prefix_job_id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        5
    );
    let (status, profiles) = at(&app, "GET", "/api/webchat/v2/prefixes", None, true).await;
    assert_eq!(status, StatusCode::OK, "{profiles}");
    assert_eq!(
        profiles["prefixes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "base-prompt",
            "defensive",
            "homeassistant",
            "tomedo",
            "sempai",
            "prefix-maker"
        ]
    );
    for profile in profiles["prefixes"].as_array().unwrap() {
        assert_eq!(
            profile["profile_revision"],
            if profile["name"] == "prefix-maker" {
                "2"
            } else {
                "1"
            }
        );
        assert!(!profile["description"].as_str().unwrap().is_empty());
        if profile["name"] == "base-prompt" {
            assert_eq!(profile["generation_status"], "legacy-unverified");
        } else {
            assert_eq!(profile["generation_available"], false);
            assert_eq!(profile["generation_status"], "prerequisites-required");
            assert_eq!(profile["fingerprint"], Value::Null);
        }
    }
    assert_eq!(
        at(&app, "GET", "/api/webchat/v2/prefixes", None, false)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        at(
            &app,
            "POST",
            "/api/webchat/v2/prefixes/sempai/regenerate",
            None,
            true
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        at(
            &app,
            "POST",
            "/api/webchat/v2/prefixes/unknown-profile/regenerate",
            None,
            true
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    // Registry-owned metadata edits survive migration reruns; no compiler path
    // supplied here can enable generation or alter the legacy bundle pointer.
    let registry_client = pool.get().await.unwrap();
    registry_client.execute("UPDATE reborn_prefix_profiles SET label='Operator label',revision=revision+1 WHERE name='sempai'", &[]).await.unwrap();
    drop(registry_client);
    brassclaw_pg::migrations::run_migrations(&pool)
        .await
        .unwrap();
    let profiles = at(&app, "GET", "/api/webchat/v2/prefixes", None, true)
        .await
        .1;
    let sempai = profiles["prefixes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "sempai")
        .unwrap();
    assert_eq!(sempai["label"], "Operator label");
    assert_eq!(sempai["profile_revision"], "2");
    // Exercise the actual source compiler and its package exporter. No invented
    // successful manifests, provider results or cache qualification receipts.
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let inventory_path = root.path().join("prefix-inventory.json");
    let source_records = ["skills.md","recipe.md","tools.md","toolskills.md","CLAUDE.md","AGENTS.md","simplified_v3.md"].into_iter().map(|path|json!({"path":path,"kind":"document","disposition":"included","reason":"complete current source"})).collect::<Vec<_>>();
    let inventory = json!({"format":"brassclaw-prefix-source-inventory/1","release":"native-storage-acceptance", "sources":source_records});
    std::fs::write(&inventory_path, inventory.to_string()).unwrap();
    let compiler = checkout.join("scripts/prefix/base-promt-compiler.py");
    let source_root = root.path().join("prefix-sources");
    let compiled_root = root.path().join("prefix-compiled");
    let python = |args: Vec<String>| {
        let output = oracle_python().arg(&compiler).args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let source = python(vec![
        "--inventory".into(),
        inventory_path.display().to_string(),
        "--source-root".into(),
        checkout.display().to_string(),
        "--output".into(),
        source_root.display().to_string(),
    ]);
    let compiled = python(vec![
        "--package".into(),
        source_root
            .join(source["source_package"].as_str().unwrap())
            .display()
            .to_string(),
        "--output".into(),
        compiled_root.display().to_string(),
    ]);
    let exported_path = root.path().join("prefix-generation.json");
    let exported = python(vec![
        "--export-generation".into(),
        compiled_root
            .join(compiled["generation"].as_str().unwrap())
            .display()
            .to_string(),
        "--output".into(),
        exported_path.display().to_string(),
    ]);
    let generation_bytes = std::fs::read_to_string(&exported_path).unwrap();
    {
        use brassclaw_host_runtime::PrefixGenerationBackend;
        let backend =
            crate::pg_prefix_generation_backend::PgPrefixGenerationBackend::new(pool.clone());
        let scope = brassclaw_host_api::ResourceScope::system();
        let receipt = backend
            .retain("base-prompt", 1, &generation_bytes, &scope)
            .await
            .unwrap();
        assert_eq!(receipt["qualification"], "quarantined");
        assert_eq!(
            receipt["profile_revision"],
            json!(1),
            "native retained export stays numeric"
        );
        let reread = backend
            .read(
                "base-prompt",
                receipt["generation"].as_str().unwrap(),
                &scope,
            )
            .await
            .unwrap();
        assert_eq!(reread["generation_bytes"], generation_bytes);
        assert_eq!(
            reread["summary"]["profile_revision"],
            json!(1),
            "native read export stays numeric"
        );
        assert!(
            backend
                .retain("base-prompt", 2, &generation_bytes, &scope)
                .await
                .is_err()
        );
    }
    let import_request = json!({"expected_profile_revision":1,"generation_bytes":generation_bytes});
    let route = "/api/webchat/v2/prefixes/base-prompt/generations";
    assert_eq!(
        at(&app, "POST", route, Some(&import_request), false)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, retained) = at(&app, "POST", route, Some(&import_request), true).await;
    assert_eq!(status, StatusCode::OK, "{retained}");
    assert_eq!(retained["summary"]["qualification"], "quarantined");
    assert_eq!(
        retained["summary"]["generation"],
        exported["generation_package"]
    );
    assert_eq!(retained["generation_bytes"], generation_bytes);
    let detail_route = format!(
        "{route}/{}",
        exported["generation_package"].as_str().unwrap()
    );
    assert_eq!(at(&app, "GET", &detail_route, None, true).await.1, retained);
    assert!(
        at(&app, "GET", route, None, true)
            .await
            .1
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["generation"] == exported["generation_package"])
    );
    history_http_tests::exercise(&app, Some(&exported_path)).await;
    let storage_client = pool.get().await.unwrap();
    let legacy_count: i64 = storage_client
        .query_one("SELECT count(*) FROM reborn_basic_prompt_store", &[])
        .await
        .unwrap()
        .get(0);
    storage_client
        .execute(
            "UPDATE reborn_prefix_profiles SET revision=revision+1 WHERE name='base-prompt'",
            &[],
        )
        .await
        .unwrap();
    // Lost-response retry does not mutate/re-import an already retained package.
    assert_eq!(
        at(&app, "POST", route, Some(&import_request), true).await.1,
        retained
    );
    let mut conflicting = import_request.clone();
    conflicting["expected_profile_revision"] = json!(2);
    assert_eq!(
        at(&app, "POST", route, Some(&conflicting), true).await.0,
        StatusCode::CONFLICT
    );
    let mut corrupt = import_request.clone();
    let mut package: Value = serde_json::from_str(&generation_bytes).unwrap();
    package["artifacts"]["context.md"]["data"] = json!("dGFtcGVyZWQ=");
    corrupt["generation_bytes"] = json!(package.to_string());
    assert_eq!(
        at(&app, "POST", route, Some(&corrupt), true).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        storage_client
            .query_one("SELECT count(*) FROM reborn_basic_prompt_store", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        legacy_count
    );
    assert!(
        storage_client
            .execute(
                "UPDATE reborn_prefix_generations SET generation_bytes='tampered'",
                &[]
            )
            .await
            .is_err()
    );
    assert!(
        storage_client
            .execute("DELETE FROM reborn_prefix_generations", &[])
            .await
            .is_err()
    );
    drop(storage_client);
    let sources_route = "/api/webchat/v2/prefixes/base-prompt/sources";
    let registration_request = json!({"reference":"https://example.org/versioned/docs?v=3#interface","title":"Versioned reference","applicability":"prefix source registration acceptance","required":true});
    assert_eq!(
        at(
            &app,
            "POST",
            sources_route,
            Some(&registration_request),
            false
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, registration) = at(
        &app,
        "POST",
        sources_route,
        Some(&registration_request),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{registration}");
    assert_eq!(registration["availability"], "not-checked");
    assert_eq!(registration["review_status"], "pending-collection-review");
    assert_eq!(registration["reference"], registration_request["reference"]);
    assert_eq!(
        at(
            &app,
            "POST",
            sources_route,
            Some(&registration_request),
            true
        )
        .await
        .1,
        registration
    );
    let mut conflicting = registration_request.clone();
    conflicting["title"] = json!("different meaning");
    assert_eq!(
        at(&app, "POST", sources_route, Some(&conflicting), true)
            .await
            .0,
        StatusCode::CONFLICT
    );
    for reference in [
        "file:///etc/passwd",
        "https://secret:credential@example.org/docs",
        "https://example.org/docs?access_token=private",
    ] {
        let mut invalid = registration_request.clone();
        invalid["reference"] = json!(reference);
        assert_eq!(
            at(&app, "POST", sources_route, Some(&invalid), true)
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    let (status, inventory) = at(&app, "GET", sources_route, None, true).await;
    assert_eq!(status, StatusCode::OK, "{inventory}");
    assert_eq!(inventory["registered"], json!([registration]));
    assert_eq!(inventory["generation"], Value::Null);
    assert!(inventory["used"].as_array().unwrap().is_empty());
    let (status, used_sources) = at(
        &app,
        "GET",
        &format!(
            "{sources_route}?generation={}",
            exported["generation_package"].as_str().unwrap()
        ),
        None,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{used_sources}");
    assert_eq!(used_sources["used"].as_array().unwrap().len(), 7);
    assert!(
        used_sources["used"]
            .as_array()
            .unwrap()
            .iter()
            .all(|source| source["archived"] == true)
    );
    let source_client = pool.get().await.unwrap();
    assert_eq!(
        source_client
            .query_one(
                "SELECT count(*) FROM reborn_prefix_source_observations",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0,
        "registration cannot fabricate availability observations"
    );
    assert!(
        source_client
            .execute("DELETE FROM reborn_prefix_sources", &[])
            .await
            .is_err()
    );
    drop(source_client);
    exercise_prefix_maker_import(&app, &checkout, root.path()).await;
    exercise_prefix_model_records(&model_app, &pool, &runtime, prefix_job_id, sempai_job_id).await;
    history_http_tests::exercise(&model_app, None).await;
    let initial = at(&app, "GET", "/api/webchat/v2/catalogue", None, true)
        .await
        .1;
    let mut subjects = Vec::new();
    let roots = [Uuid::new_v4(), Uuid::new_v4()];
    for (root, label) in roots.iter().zip(["alpha", "beta"]) {
        let proposal = json!({"submission_id":Uuid::new_v4(),"candidate_bytes":make_recipe(*root,label).exact_bytes(),"base":null,"dependencies":dependencies});
        let (status, receipt) = at(
            &app,
            "POST",
            "/api/webchat/v2/component-review-submissions",
            Some(&proposal),
            true,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{receipt}");
        subjects.push(json!({"subject":{"submission_id":receipt["submission_id"],"subject_checksum":receipt["subject_checksum"]},"workflow_class":"deterministic","association_approvals":{}}));
    }
    let client = pool.get().await.unwrap();
    let evidence_before: i64 = client
        .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
        .await
        .unwrap()
        .get(0);
    let effects_before: i64 = client
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get(0);
    // Export is a projection of the owner's selected graph, never staged drafts.
    let initial_path = format!(
        "/api/webchat/v2/catalogue/{}/sources",
        initial["generation"].as_str().unwrap()
    );
    assert_eq!(
        at(&app, "GET", &initial_path, None, false).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert!(
        at(
            &app,
            "GET",
            "/api/webchat/v2/catalogue/not-a-generation/sources",
            None,
            true
        )
        .await
        .0
        .is_client_error()
    );
    let (status, initial_sources) = at(&app, "GET", &initial_path, None, true).await;
    assert_eq!(status, StatusCode::OK, "{initial_sources}");
    assert_eq!(
        initial_sources["format"],
        "brassclaw-catalogue-source-export/1"
    );
    assert_eq!(
        initial_sources["coverage"],
        "active-ordinary-workflows-and-retained-root"
    );
    assert_eq!(
        at(&app, "GET", &initial_path, None, true).await.1,
        initial_sources
    );
    for component in initial_sources["components"].as_array().unwrap() {
        let r = &component["reference"];
        let uuid = Uuid::parse_str(r["uuid"].as_str().unwrap()).unwrap();
        assert!(
            !roots.contains(&uuid),
            "pending proposal must not be exported"
        );
        let version = i64::try_from(r["version"].as_u64().unwrap()).unwrap();
        let row = client.query_one("SELECT revision_bytes,checksum,class_code FROM reborn_component_revisions WHERE component_id=$1 AND version=$2", &[&uuid, &version]).await.unwrap();
        assert_eq!(component["revision_bytes"], row.get::<_, String>(0));
        assert_eq!(r["checksum"], row.get::<_, String>(1));
        assert_eq!(r["class_code"], i32::from(row.get::<_, i16>(2)));
    }
    let selected = catalogue.active.read().await.clone();
    let mut expected_ids =
        std::collections::BTreeSet::from([catalogue.root_reference.uuid.to_string()]);
    let mut expected_selections = std::collections::BTreeSet::new();
    for program in std::iter::once(&catalogue.reply)
        .chain(std::iter::once(&catalogue.history))
        .chain(selected.programs.iter())
    {
        let instruction = program.program().inputs().instruction();
        expected_ids.extend(
            instruction
                .snapshot()
                .revisions()
                .keys()
                .map(ToString::to_string),
        );
        expected_selections.insert(
            instruction
                .retained_selection()
                .unwrap()
                .exact_bytes()
                .to_owned(),
        );
    }
    assert_eq!(
        initial_sources["components"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["reference"]["uuid"].as_str().unwrap().to_owned())
            .collect::<std::collections::BTreeSet<_>>(),
        expected_ids
    );
    assert_eq!(
        initial_sources["selections"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_owned())
            .collect::<std::collections::BTreeSet<_>>(),
        expected_selections
    );
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        effects_before
    );
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        evidence_before
    );
    let request = json!({"activation_id":Uuid::new_v4(),"expected_generation":initial["generation"],"recipes":[subjects[0]]});
    assert_eq!(
        at(
            &app,
            "POST",
            "/api/webchat/v2/component-deployment",
            Some(&request),
            false
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let mut forged = request.clone();
    forged["approved"] = json!(true);
    assert!(
        at(
            &app,
            "POST",
            "/api/webchat/v2/component-deployment",
            Some(&forged),
            true
        )
        .await
        .0
        .is_client_error()
    );
    let (status, plan) = at(
        &app,
        "POST",
        "/api/webchat/v2/component-deployment",
        Some(&request),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{plan}");
    assert_eq!(plan["catalogue_activated"], false);
    assert_eq!(
        at(&app, "GET", "/api/webchat/v2/catalogue", None, true)
            .await
            .1["generation"],
        initial["generation"]
    );
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        effects_before
    );
    let first = plan["activation"].clone();
    let mut tampered = first.clone();
    tampered["deployment_subjects"][0]["subject_checksum"] = json!("f".repeat(64));
    assert_eq!(
        at(
            &app,
            "POST",
            "/api/webchat/v2/catalogue",
            Some(&tampered),
            true
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let mut tampered = first.clone();
    tampered["recipes"][0]["components"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(
        at(
            &app,
            "POST",
            "/api/webchat/v2/catalogue",
            Some(&tampered),
            true
        )
        .await
        .0
        .is_client_error()
    );
    let (status, receipt) = at(
        &app,
        "POST",
        "/api/webchat/v2/catalogue",
        Some(&first),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["catalogue_activated"], true);
    assert_eq!(
        at(&app, "GET", &initial_path, None, true).await.0,
        StatusCode::CONFLICT
    );
    let (status, sources) = at(
        &app,
        "GET",
        &format!(
            "/api/webchat/v2/catalogue/{}/sources",
            receipt["generation"].as_str().unwrap()
        ),
        None,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sources}");
    let ids: Vec<_> = sources["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["reference"]["uuid"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&roots[0].to_string().as_str()));
    assert!(!ids.contains(&roots[1].to_string().as_str()));
    assert_eq!(sources["generation"], receipt["generation"]);
    assert_ne!(
        sources["catalogue_bytes"],
        initial_sources["catalogue_bytes"]
    );

    assert_eq!(
        at(
            &app,
            "POST",
            "/api/webchat/v2/catalogue",
            Some(&first),
            true
        )
        .await
        .1,
        receipt
    );
    let mut second_request = request.clone();
    second_request["activation_id"] = json!(Uuid::new_v4());
    second_request["recipes"] = json!([subjects[1]]);
    assert_eq!(
        at(
            &app,
            "POST",
            "/api/webchat/v2/component-deployment",
            Some(&second_request),
            true
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    second_request["expected_generation"] = receipt["generation"].clone();
    let (status, second) = at(
        &app,
        "POST",
        "/api/webchat/v2/component-deployment",
        Some(&second_request),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["preserved_recipe_ids"], json!([roots[0]]));
    assert_eq!(second["activation"]["recipes"].as_array().unwrap().len(), 2);
    let mut dropped = second["activation"].clone();
    dropped["recipes"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["recipe_id"] != json!(roots[0]));
    assert!(
        at(
            &app,
            "POST",
            "/api/webchat/v2/catalogue",
            Some(&dropped),
            true
        )
        .await
        .0
        .is_client_error()
    );
    let (status, second_receipt) = at(
        &app,
        "POST",
        "/api/webchat/v2/catalogue",
        Some(&second["activation"]),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second_receipt}");
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        evidence_before
    );
    let stored:String=client.query_one("SELECT request_bytes FROM brassclaw_monty_catalogue_activations WHERE activation_id=$1",&[&Uuid::parse_str(first["activation_id"].as_str().unwrap()).unwrap()]).await.unwrap().get(0);
    assert_eq!(
        serde_json::from_str::<Value>(&stored).unwrap()["deployment_subjects"],
        first["deployment_subjects"]
    );
    for (root, label) in roots.iter().zip(["alpha", "beta"]) {
        let chat = runtime.new_conversation().await.unwrap();
        let reply = runtime
            .send_user_message(
                &chat,
                &format!("deployment {label} reply literal ' ü {{source}}"),
            )
            .await
            .unwrap();
        assert_eq!(reply.status, TurnStatus::Completed);
        assert_eq!(reply.text.as_deref(), Some("literal ' ü {source}"));
        let outcome: Value = client
            .query_one(
                "SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'",
                &[&reply.run_id.as_uuid()],
            )
            .await
            .unwrap()
            .get(0);
        assert!(
            outcome.to_string().contains(&root.to_string()),
            "ordinary matched turn must retain the deployed Recipe selection"
        );
    }
    // A successor must be based on the exact active root. Original subject
    // retention cannot authorize replacing its newer activated revision.
    let base = first["recipes"][0]["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["uuid"] == json!(roots[0]))
        .unwrap()
        .clone();
    let successor = json!({"submission_id":Uuid::new_v4(),"candidate_bytes":make_recipe(roots[0],"alpha").exact_bytes(),"base":base,"dependencies":dependencies});
    let (status, successor) = at(
        &app,
        "POST",
        "/api/webchat/v2/component-review-submissions",
        Some(&successor),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{successor}");
    let upgrade = json!({"activation_id":Uuid::new_v4(),"expected_generation":second_receipt["generation"],"recipes":[{"subject":{"submission_id":successor["submission_id"],"subject_checksum":successor["subject_checksum"]},"workflow_class":"deterministic","association_approvals":{}}]});
    let (status, upgraded) = at(
        &app,
        "POST",
        "/api/webchat/v2/component-deployment",
        Some(&upgrade),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{upgraded}");
    let (status, upgraded) = at(
        &app,
        "POST",
        "/api/webchat/v2/catalogue",
        Some(&upgraded["activation"]),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{upgraded}");
    let mut stale = request.clone();
    stale["activation_id"] = json!(Uuid::new_v4());
    stale["expected_generation"] = upgraded["generation"].clone();
    assert_eq!(
        at(
            &app,
            "POST",
            "/api/webchat/v2/component-deployment",
            Some(&stale),
            true
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    // Neither the old receipt nor the subject pins republish withdrawn work.
    let withdraw = json!({"activation_id":Uuid::new_v4(),"expected_generation":upgraded["generation"],"recipes":[]});
    assert_eq!(
        at(
            &app,
            "POST",
            "/api/webchat/v2/catalogue",
            Some(&withdraw),
            true
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        at(
            &app,
            "POST",
            "/api/webchat/v2/catalogue",
            Some(&first),
            true
        )
        .await
        .1["catalogue_activated"],
        false
    );
    // Optional caller-selected artifact for cross-language compiler acceptance.
    // These are actual authenticated HTTP bytes, never synthetic approval data.
    if let Some(path) = std::env::var_os("BRASSCLAW_TEST_CATALOGUE_EXPORT") {
        std::fs::write(path, serde_json::to_vec(&sources).unwrap()).unwrap();
    }
    drop(client);
    exercise_prefix_revision_boundaries(
        &model_app,
        &pool,
        &runtime,
        &generation_bytes,
        root.path(),
    )
    .await;
    drop(app);
    drop(catalogue);
    shutdown_shared_runtime(runtime).await.unwrap();
    transport.shutdown().await.unwrap();
}

async fn exercise_prefix_model_records(
    app: &axum::Router,
    pool: &Arc<brassclaw_pg::PgPool>,
    runtime: &Arc<RebornRuntime>,
    prefix_job_id: Uuid,
    sempai_job_id: Uuid,
) {
    let catalogue_route = "/api/webchat/v2/prefix-model-designs";
    let capture_route = "/api/webchat/v2/prefix-model-specifications";
    assert_eq!(
        at(app, "GET", catalogue_route, None, false).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, empty) = at(app, "GET", catalogue_route, None, true).await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty["design_generation_available"], false);
    let (status, observed) = at(app, "GET", "/api/webchat/v2/llm/providers", None, true).await;
    assert_eq!(status, StatusCode::OK, "{observed}");
    let provider = json!({"expected_selection_ref":observed["role_selection_refs"]["kohai"],"mutation_id":uuid::Uuid::new_v4(),"id":"prefix-model-acceptance","adapter":"open_ai_completions","base_url":"http://127.0.0.1:1/v1","default_model":"exact-model","model_token_limits":{"configured-inactive-model":{"max_prefix_tokens":100000,"max_prompt_tokens":14688}},"set_active":true});
    let (status, saved) = at(
        app,
        "POST",
        "/api/webchat/v2/llm/providers",
        Some(&provider),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let mutation_id = provider["mutation_id"].as_str().unwrap();
    let original_route = format!("/api/webchat/v2/llm/mutations/{mutation_id}");
    let reconcile_route = format!("{original_route}/reconcile");
    assert_eq!(
        saved["mutation_receipt"]["mutation_id"],
        provider["mutation_id"]
    );
    assert_eq!(
        at(app, "GET", &original_route, None, false).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, original) = at(app, "GET", &original_route, None, true).await;
    assert_eq!(status, StatusCode::OK, "{original}");
    assert_eq!(original, saved["mutation_receipt"]);
    assert_eq!(
        at(app, "POST", &reconcile_route, None, false).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, reconciled) = at(app, "POST", &reconcile_route, None, true).await;
    assert_eq!(status, StatusCode::OK, "{reconciled}");
    assert_eq!(reconciled["mutation_receipt"], original);
    let (status, replayed) = at(
        app,
        "POST",
        "/api/webchat/v2/llm/providers",
        Some(&provider),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed["mutation_receipt"], original);
    let mut conflict = provider.clone();
    conflict["name"] = json!("different original payload");
    assert_eq!(
        at(
            app,
            "POST",
            "/api/webchat/v2/llm/providers",
            Some(&conflict),
            true
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let unknown_route = format!("/api/webchat/v2/llm/mutations/{}", Uuid::new_v4());
    assert_eq!(
        at(app, "GET", &unknown_route, None, true).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        at(
            app,
            "POST",
            &format!("{unknown_route}/reconcile"),
            None,
            true
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    let (status, catalogue) = at(app, "GET", catalogue_route, None, true).await;
    assert_eq!(status, StatusCode::OK, "{catalogue}");
    let target = catalogue["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["provider_id"] == "prefix-model-acceptance")
        .unwrap();
    assert_eq!(target["consumer"], "kohai");
    let request = json!({"provider_id":target["provider_id"],"expected_provider_revision":target["provider_revision"],"model":target["model"]});
    assert_eq!(
        at(app, "POST", capture_route, Some(&request), false)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, specification) = at(app, "POST", capture_route, Some(&request), true).await;
    assert_eq!(status, StatusCode::OK, "{specification}");
    let bytes: Value =
        serde_json::from_str(specification["specification_bytes"].as_str().unwrap()).unwrap();
    assert_eq!(specification["qualification"], "unknown-model-artifacts");
    for field in [
        "weights_identity",
        "config_identity",
        "tokenizer_identity",
        "template_identity",
        "architecture",
        "attention_recurrent_state",
        "runtime_cache",
    ] {
        assert_eq!(bytes[field], Value::Null, "{field}");
    }
    assert_eq!(
        at(app, "POST", capture_route, Some(&request), true).await.1,
        specification
    );
    setup_http_tests::exercise(app, &specification).await;
    exercise_prefix_model_artifacts(
        app,
        pool,
        &specification,
        runtime,
        prefix_job_id,
        sempai_job_id,
    )
    .await;
    let proposal_bytes = json!({"format":"brassclaw-prefix-design-proposal/1","profile":"sempai","specification":specification["id"],"design":{"adapter":"ornith-qwen-system/1","evidence_view":"literal-reference/v5-contextual-sharing","source_selection":"complete-mandatory","reserve_tokens":8192},"rationale":"Recorded proposal <img src=x onerror='window.prefixOriginalExecuted=true'>; no quality or architecture qualification asserted."}).to_string();
    let proposal_request =
        json!({"specification":specification["id"],"proposal_bytes":proposal_bytes});
    let route = "/api/webchat/v2/prefixes/sempai/design-proposals";
    let (status, proposal) = at(app, "POST", route, Some(&proposal_request), true).await;
    assert_eq!(status, StatusCode::OK, "{proposal}");
    assert_eq!(proposal["qualification"], "unaccepted-proposal");
    assert_eq!(
        at(app, "POST", route, Some(&proposal_request), true)
            .await
            .1,
        proposal
    );
    let mut obsolete: Value = serde_json::from_str(&proposal_bytes).unwrap();
    obsolete["design"]["evidence_view"] = json!("literal-reference/v1");
    assert_eq!(
        at(
            app,
            "POST",
            route,
            Some(&json!({"specification":specification["id"],
        "proposal_bytes":obsolete.to_string()})),
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST,
        "new Sempai proposals must use the actual supported context-aware adapter"
    );
    let mut invalid: Value = serde_json::from_str(&proposal_bytes).unwrap();
    invalid["design"]["source_selection"] = json!("omit-whatever-does-not-fit");
    assert_eq!(
        at(
            app,
            "POST",
            route,
            Some(
                &json!({"specification":specification["id"],"proposal_bytes":invalid.to_string()})
            ),
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, observed) = at(app, "GET", "/api/webchat/v2/llm/providers", None, true).await;
    assert_eq!(status, StatusCode::OK, "{observed}");
    let revision = observed["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "prefix-model-acceptance")
        .unwrap()["revision"]
        .clone();
    let selection = json!({"expected_selection_ref":observed["role_selection_refs"]["kohai"],"expected_revision":revision,"mutation_id":uuid::Uuid::new_v4(),"provider_id":"prefix-model-acceptance","model":"different-model","role":"kohai"});
    let (status, result) = at(
        app,
        "POST",
        "/api/webchat/v2/llm/active",
        Some(&selection),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let (status, catalogue) = at(app, "GET", catalogue_route, None, true).await;
    assert_eq!(status, StatusCode::OK, "{catalogue}");
    assert_eq!(catalogue["specifications"][0], specification);
    assert_eq!(
        catalogue["proposals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == proposal["id"]),
        Some(&proposal)
    );
    let target = catalogue["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["provider_id"] == "prefix-model-acceptance")
        .unwrap();
    assert_eq!(target["model"], "different-model");
    assert_eq!(
        target["configured_context_window"],
        Value::Null,
        "a default-model context must not qualify an override"
    );
    assert_eq!(
        at(app, "POST", capture_route, Some(&request), true).await.1,
        specification,
        "a lost-response retry retains the old contract; it cannot capture the new target"
    );
    let mut uncaptured = request.clone();
    uncaptured["model"] = json!("not-an-active-model");
    assert_eq!(
        at(app, "POST", capture_route, Some(&uncaptured), true)
            .await
            .0,
        StatusCode::CONFLICT
    );
    let client = pool.get().await.unwrap();
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM reborn_prefix_design_acceptances", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    for table in [
        "reborn_prefix_model_specifications",
        "reborn_prefix_design_proposals",
    ] {
        assert!(
            client
                .execute(&format!("DELETE FROM {table}"), &[])
                .await
                .is_err()
        );
    }
    drop(client);
    let before = at(app, "GET", "/api/webchat/v2/llm/providers", None, true)
        .await
        .1;
    let catalogue = at(app, "GET", catalogue_route, None, true).await.1;
    let target = catalogue["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|target| {
            target["provider_id"] == "prefix-model-acceptance"
                && target["model"] == "configured-inactive-model"
        })
        .unwrap();
    assert_eq!(target["consumer"], "configured");
    assert_eq!(target["configured_context_window"], Value::Null);
    let request = json!({"provider_id":target["provider_id"],
        "expected_provider_revision":target["provider_revision"],"model":target["model"]});
    let mut stale = request.clone();
    stale["expected_provider_revision"] = json!(
        target["provider_revision"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap()
            + 1
    );
    assert_eq!(
        at(app, "POST", capture_route, Some(&stale), true).await.0,
        StatusCode::CONFLICT
    );
    let (status, inactive) = at(app, "POST", capture_route, Some(&request), true).await;
    assert_eq!(status, StatusCode::OK, "{inactive}");
    let contract: Value =
        serde_json::from_str(inactive["specification_bytes"].as_str().unwrap()).unwrap();
    assert_eq!(
        contract["model_token_limits"],
        json!({"max_prefix_tokens":100000,"max_prompt_tokens":14688})
    );
    assert_eq!(contract["configured_context_window"], Value::Null);
    assert_eq!(inactive["model"], "configured-inactive-model");
    assert_eq!(
        at(app, "POST", capture_route, Some(&request), true).await.1,
        inactive
    );
    let after = at(app, "GET", "/api/webchat/v2/llm/providers", None, true)
        .await
        .1;
    assert_eq!(
        before, after,
        "inactive model capture must not edit providers or role selections"
    );
    let passive_provider = json!({"mutation_id":Uuid::new_v4(),"id":"prefix-passive-provider",
        "adapter":"open_ai_completions","base_url":"http://127.0.0.1:1/v1",
        "default_model":"configured-inactive-model",
        "model_token_limits":{"configured-inactive-model":{"max_prefix_tokens":64000,"max_prompt_tokens":32768}},
        "set_active":false});
    let (status, passive) = at(
        app,
        "POST",
        "/api/webchat/v2/llm/providers",
        Some(&passive_provider),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{passive}");
    assert_eq!(passive["kohai_active"], before["kohai_active"]);
    assert_eq!(passive["sempai_active"], before["sempai_active"]);
    let catalogue = at(app, "GET", catalogue_route, None, true).await.1;
    let target = catalogue["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|target| target["provider_id"] == "prefix-passive-provider")
        .unwrap();
    assert_eq!(target["consumer"], "configured");
    assert_eq!(target["configured_context_window"], Value::Null);
    let request = json!({"provider_id":target["provider_id"],
        "expected_provider_revision":target["provider_revision"],"model":target["model"]});
    let (status, specification) = at(app, "POST", capture_route, Some(&request), true).await;
    assert_eq!(status, StatusCode::OK, "{specification}");
    assert_ne!(
        specification["id"], inactive["id"],
        "same model name on another provider is another target"
    );
    let contract: Value =
        serde_json::from_str(specification["specification_bytes"].as_str().unwrap()).unwrap();
    assert_eq!(contract["configured_context_window"], Value::Null);
    assert_eq!(
        contract["model_token_limits"],
        json!({"max_prefix_tokens":64000,"max_prompt_tokens":32768})
    );
}

async fn prefix_metadata_llm_service(
    pool: Arc<brassclaw_pg::PgPool>,
    tenant: &str,
) -> Arc<crate::RebornLlmConfigService> {
    // This fixture's runtime deliberately overrides its gateway and has no live
    // provider-reload handle. Attach the actual configuration/reload owners for
    // operator model-selection acceptance; no inference response is fabricated.
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            pool.as_ref().clone(),
            secrecy::SecretString::from("prefix-metadata-native-retained-master-key"),
            tenant,
        )
        .unwrap(),
    ));
    let provider = Arc::new(brassclaw_llm::SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let handle = Arc::new(brassclaw_llm::LlmReloadHandle::new(provider, None));
    let reload = Arc::new(crate::llm_reload::RebornLlmReloadAdapter::new(
        handle,
        Arc::new(brassclaw_llm::SessionManager::new(Default::default())),
        keys.clone(),
        pool.clone(),
        tenant,
    ));
    Arc::new(
        crate::RebornLlmConfigService::new(
            keys,
            pool.clone(),
            Arc::new(crate::PgProviderRepo::new(pool.as_ref().clone(), tenant)),
            tenant,
        )
        .with_reload_trigger(reload),
    )
}

async fn exercise_prefix_maker_import(
    app: &axum::Router,
    checkout: &std::path::Path,
    temporary: &std::path::Path,
) {
    let inventory_path = temporary.join("prefix-maker-inventory.json");
    let records = ["skills.md","recipe.md","tools.md","toolskills.md","CLAUDE.md","AGENTS.md","simplified_v3.md","scripts/prefix/prefix-maker-reference.md"].into_iter().map(|path|json!({"path":path,"kind":"document","disposition":"included","reason":"complete Prefix-Maker development reference"})).collect::<Vec<_>>();
    std::fs::write(&inventory_path, json!({"format":"brassclaw-prefix-source-inventory/1","release":"native-prefix-maker-storage", "sources":records}).to_string()).unwrap();
    let compiler = checkout.join("scripts/prefix/prefix-maker-compiler.py");
    let run = |args: Vec<String>| {
        let output = oracle_python().arg(&compiler).args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let sources = temporary.join("maker-sources");
    let compiled = temporary.join("maker-compiled");
    let source = run(vec![
        "--inventory".into(),
        inventory_path.display().to_string(),
        "--source-root".into(),
        checkout.display().to_string(),
        "--output".into(),
        sources.display().to_string(),
    ]);
    let generation = run(vec![
        "--package".into(),
        sources
            .join(source["source_package"].as_str().unwrap())
            .display()
            .to_string(),
        "--output".into(),
        compiled.display().to_string(),
    ]);
    let transfer = temporary.join("maker-transfer.json");
    run(vec![
        "--export-generation".into(),
        compiled
            .join(generation["generation"].as_str().unwrap())
            .display()
            .to_string(),
        "--output".into(),
        transfer.display().to_string(),
    ]);
    let bytes = std::fs::read_to_string(transfer).unwrap();
    let (status, result) = at(
        app,
        "POST",
        "/api/webchat/v2/prefixes/prefix-maker/generations",
        Some(&json!({"expected_profile_revision":2,"generation_bytes":bytes})),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["summary"]["qualification"], "quarantined");
    let (status, inventory) = at(
        app,
        "GET",
        &format!(
            "/api/webchat/v2/prefixes/prefix-maker/sources?generation={}",
            result["summary"]["generation"].as_str().unwrap()
        ),
        None,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{inventory}");
    assert_eq!(inventory["used"].as_array().unwrap().len(), 8);
    let deduplicated = run(vec![
        "--package".into(),
        sources
            .join(source["source_package"].as_str().unwrap())
            .display()
            .to_string(),
        "--output".into(),
        compiled.display().to_string(),
        "--evidence-view".into(),
        "literal-reference/v2-deduplicated".into(),
    ]);
    assert_ne!(deduplicated["generation"], generation["generation"]);
    let transfer = temporary.join("maker-deduplicated-transfer.json");
    run(vec![
        "--export-generation".into(),
        compiled
            .join(deduplicated["generation"].as_str().unwrap())
            .display()
            .to_string(),
        "--output".into(),
        transfer.display().to_string(),
    ]);
    let (status, retained) = at(app, "POST", "/api/webchat/v2/prefixes/prefix-maker/generations", Some(&json!({"expected_profile_revision":2,"generation_bytes":std::fs::read_to_string(transfer).unwrap()})), true).await;
    assert_eq!(status, StatusCode::OK, "{retained}");
    assert_eq!(retained["summary"]["qualification"], "quarantined");
}

/// Unreviewed retention fixtures exercise immutable selection only. They never
/// manufacture approval or enter an activated catalogue or executable Recipe.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_retained_revision_selection_preserves_deep_graphs_and_old_revisions() {
    use brassclaw_skills::{
        component_revision::ComponentRevisionDraft, revision_store::PgComponentRevisionStore,
    };
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let ids: Vec<_> = (0..192).map(|_| uuid::Uuid::new_v4()).collect();
    let draft = |id, dependencies: &[uuid::Uuid]| {
        ComponentRevisionDraft::from_json(
            &serde_json::json!({
                "format":"component-revision/1", "uuid":id, "class_code":22,
                "document":{"label":"unreviewed retention fixture"},
                "dependencies":dependencies, "association":null,
            })
            .to_string(),
        )
        .unwrap()
    };
    let mut client = rig.pool.get().await.unwrap();
    let tx = client.transaction().await.unwrap();
    let mut refs = Vec::new();
    for (index, id) in ids.iter().enumerate() {
        let dependencies = ids.get(index + 1).map(std::slice::from_ref).unwrap_or(&[]);
        refs.push(
            PgComponentRevisionStore::stage_in_transaction(&tx, &draft(*id, dependencies), 0)
                .await
                .unwrap(),
        );
    }
    tx.commit().await.unwrap();
    drop(client);
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let selected = store
        .read_exact(
            &[ids[0]],
            &refs,
            brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
        )
        .await
        .unwrap();
    assert_eq!(selected.revisions().len(), ids.len());
    for reference in &refs {
        assert_eq!(
            selected.revisions()[&reference.uuid].reference(),
            *reference
        );
    }
    let successor = store.stage(&draft(ids[0], &[]), 1).await.unwrap();
    assert_eq!(successor.version, 2);
    assert_ne!(successor.checksum, refs[0].checksum);
    assert_eq!(
        store
            .read_exact(
                &[ids[0]],
                &[successor],
                brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph()
            )
            .await
            .unwrap()
            .revisions()
            .len(),
        1
    );
    let original = store
        .read_exact(
            &[ids[0]],
            &refs,
            brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
        )
        .await
        .unwrap();
    assert_eq!(original.revisions().len(), ids.len());
    assert_eq!(original.revisions()[&ids[0]].reference(), refs[0]);
    let mut cyclic = refs.clone();
    *cyclic.last_mut().unwrap() = store
        .stage(&draft(*ids.last().unwrap(), &[ids[0]]), 1)
        .await
        .unwrap();
    let error = match store
        .read_exact(
            &[ids[0]],
            &cyclic,
            brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
        )
        .await
    {
        Err(error) => error,
        Ok(_) => panic!("cyclic exact selection must fail"),
    };
    assert!(error.to_string().contains("cyclic dependency graph"));
    assert_eq!(
        store
            .read_exact(
                &[ids[0]],
                &refs,
                brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph()
            )
            .await
            .unwrap()
            .revisions()
            .len(),
        ids.len()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_lifecycle_publication_isolates_equal_cursors_and_required_errors() {
    use brassclaw_turns::{
        DefaultTurnCoordinator, DefaultTurnLifecycleEventBus, LifecyclePublicationErrorPort,
        LifecyclePublishingTurnStateStore, TurnCoordinator, TurnEventKind, TurnLifecycleEventBus,
        TurnStateStore,
    };
    struct Observer {
        events: std::sync::Mutex<Vec<brassclaw_turns::TurnLifecycleEvent>>,
    }
    #[async_trait::async_trait]
    impl brassclaw_turns::TurnCommittedEventObserver for Observer {
        fn observes_event(&self, event: &brassclaw_turns::TurnLifecycleEvent) -> bool {
            event.kind == TurnEventKind::Submitted
        }
        async fn observe_committed_state(
            &self,
            _state: brassclaw_turns::TurnRunState,
        ) -> Result<(), brassclaw_turns::TurnError> {
            Ok(())
        }
        async fn observe_committed_event(
            &self,
            event: brassclaw_turns::TurnLifecycleEvent,
        ) -> Result<(), brassclaw_turns::TurnError> {
            let failed = event.scope.thread_id.as_str() == "failed-thread";
            self.events.lock().unwrap().push(event);
            if failed {
                Err(brassclaw_turns::TurnError::Unavailable {
                    reason: "deliberate required observer rejection".into(),
                })
            } else {
                Ok(())
            }
        }
    }
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let raw = Arc::new(brassclaw_turns::PgTurnStateStore::new(
        rig.pool.clone(),
        "native-lifecycle",
    ));
    let observer = Arc::new(Observer {
        events: std::sync::Mutex::new(Vec::new()),
    });
    let bus = Arc::new(DefaultTurnLifecycleEventBus::new());
    bus.subscribe_required(observer.clone()).unwrap();
    let store = Arc::new(LifecyclePublishingTurnStateStore::new(raw.clone(), bus));
    let coordinator = DefaultTurnCoordinator::new(store.clone())
        .with_lifecycle_publication_error_port(store.clone());
    let request = |thread: &str, run_id| brassclaw_turns::SubmitTurnRequest {
        scope: brassclaw_turns::TurnScope::new(
            brassclaw_host_api::TenantId::new("native-lifecycle").unwrap(),
            None,
            None,
            brassclaw_host_api::ThreadId::new(thread).unwrap(),
        ),
        actor: brassclaw_turns::TurnActor::new(
            brassclaw_host_api::UserId::new("lifecycle-operator").unwrap(),
        ),
        accepted_message_ref: brassclaw_turns::AcceptedMessageRef::new(format!("message-{thread}"))
            .unwrap(),
        source_binding_ref: brassclaw_turns::SourceBindingRef::new("native-lifecycle").unwrap(),
        reply_target_binding_ref: brassclaw_turns::ReplyTargetBindingRef::new("native-lifecycle")
            .unwrap(),
        requested_run_profile: None,
        idempotency_key: brassclaw_turns::IdempotencyKey::new(thread).unwrap(),
        received_at: chrono::Utc::now(),
        requested_run_id: Some(run_id),
        parent_run_id: None,
        subagent_depth: 0,
        spawn_tree_root_run_id: None,
    };
    let good_id = brassclaw_turns::TurnRunId::new();
    let bad_id = brassclaw_turns::TurnRunId::new();
    let good = request("good-thread", good_id);
    let bad = request("failed-thread", bad_id);
    coordinator.submit_turn(good.clone()).await.unwrap();
    assert!(coordinator.submit_turn(bad.clone()).await.is_err());
    let good_state = raw
        .get_run_state(brassclaw_turns::GetRunStateRequest {
            scope: good.scope.clone(),
            run_id: good_id,
        })
        .await
        .unwrap();
    let bad_state = raw
        .get_run_state(brassclaw_turns::GetRunStateRequest {
            scope: bad.scope.clone(),
            run_id: bad_id,
        })
        .await
        .unwrap();
    assert_eq!(
        good_state.event_cursor, bad_state.event_cursor,
        "independent PG snapshots reuse cursor sequences"
    );
    assert_eq!(
        observer.events.lock().unwrap().len(),
        2,
        "both real committed events must reach the observer"
    );
    for _ in 0..2 {
        assert!(
            store
                .lifecycle_publication_error(&bad.scope, bad_id, bad_state.event_cursor)
                .is_some(),
            "failure is not consumed by another caller"
        );
        assert!(
            store
                .lifecycle_publication_error(&good.scope, good_id, good_state.event_cursor)
                .is_none(),
            "failure stays bound to its actual event"
        );
    }
    coordinator.submit_turn(good.clone()).await.unwrap();
    assert_eq!(
        observer.events.lock().unwrap().len(),
        2,
        "acknowledged good event is not replayed"
    );
    assert!(coordinator.submit_turn(bad).await.is_err());
    assert_eq!(
        observer.events.lock().unwrap().len(),
        3,
        "only the failed event retries publication"
    );
}

async fn exercise_prefix_model_artifacts(
    app: &axum::Router,
    pool: &Arc<brassclaw_pg::PgPool>,
    specification: &Value,
    runtime: &Arc<RebornRuntime>,
    prefix_job_id: Uuid,
    sempai_job_id: Uuid,
) {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use sha2::Digest;
    let directory = std::path::PathBuf::from(
        std::env::var("BRASSCLAW_TEST_MODEL_ARTIFACT_ROOT")
            .expect("real frozen model artifacts required for native import acceptance"),
    );
    let mut files = serde_json::Map::new();
    let mut originals = std::collections::BTreeMap::new();
    for name in [
        "config.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "chat_template.jinja",
        "vocab.json",
        "merges.txt",
    ] {
        let bytes = std::fs::read(directory.join(name)).unwrap();
        files.insert(name.into(),json!({"encoding":"base64","sha256":hex::encode(sha2::Sha256::digest(&bytes)),"data":STANDARD.encode(&bytes)}));
        originals.insert(name, bytes);
    }
    let set = json!({"format":"brassclaw-prefix-model-artifact-set/1","artifacts":files});
    let request = json!({"specification":specification["id"],"artifact_set_bytes":set.to_string()});
    let route = "/api/webchat/v2/prefix-model-artifacts";
    assert_eq!(
        at(app, "POST", route, Some(&request), false).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, stored) = at(app, "POST", route, Some(&request), true).await;
    assert_eq!(status, StatusCode::OK, "{stored}");
    assert_eq!(stored["qualification"], "unverified-model-artifacts");
    assert_eq!(stored["specification"], specification["id"]);
    assert_eq!(
        at(app, "POST", route, Some(&request), true).await.1,
        stored,
        "lost-response repeat must retain the original provenance"
    );
    let detail = format!(
        "{route}/{}/{}",
        specification["id"].as_str().unwrap(),
        stored["id"].as_str().unwrap()
    );
    assert_eq!(
        at(app, "GET", &detail, None, false).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, read) = at(app, "GET", &detail, None, true).await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(read["summary"], stored);
    let observation: Value =
        serde_json::from_str(read["configuration_observation"].as_str().unwrap()).unwrap();
    assert_eq!(
        observation["format"],
        "brassclaw-prefix-model-config-observation/1"
    );
    assert_eq!(observation["qualification"], "declared-config-only");
    assert_eq!(observation["specification"], specification["id"]);
    assert_eq!(observation["artifact_set"], stored["id"]);
    assert_eq!(
        observation["config_sha256"],
        hex::encode(sha2::Sha256::digest(&originals["config.json"]))
    );
    let declared: Value = serde_json::from_slice(&originals["config.json"]).unwrap();
    assert_eq!(observation["fields"]["/model_type"], declared["model_type"]);
    assert_eq!(
        observation["fields"]["/text_config/layer_types"],
        declared["text_config"]["layer_types"]
    );
    for field in ["runtime_cache", "weights_identity", "serving_context_limit"] {
        assert!(observation[field].is_null());
    }
    pair_construction_tests::exercise(app, pool, specification, &read).await;
    let bytes = read["artifact_set_bytes"].as_str().unwrap();
    assert_eq!(
        stored["id"],
        hex::encode(sha2::Sha256::digest(bytes.as_bytes()))
    );
    let read: Value = serde_json::from_str(bytes).unwrap();
    for (name, original) in originals {
        assert_eq!(
            STANDARD
                .decode(read["artifacts"][name]["data"].as_str().unwrap())
                .unwrap(),
            original
        );
    }
    super::model_encoding_tests::assert_registered_encoding(
        runtime,
        app,
        specification["id"].as_str().unwrap(),
        stored["id"].as_str().unwrap(),
        prefix_job_id,
        sempai_job_id,
    )
    .await;
    let mut import = set.clone();
    import["format"] = json!("brassclaw-prefix-model-artifact-import/1");
    for artifact in import["artifacts"].as_object_mut().unwrap().values_mut() {
        artifact.as_object_mut().unwrap().remove("sha256");
    }
    let import =
        json!({"specification":specification["id"],"artifact_set_bytes":import.to_string()});
    assert_eq!(
        at(app, "POST", route, Some(&import), true).await.1,
        stored,
        "browser import and exact checksum transport must share one content identity"
    );
    let mut invalid = request.clone();
    invalid["origin"] = json!({"kind":"system_seed"});
    assert!(
        at(app, "POST", route, Some(&invalid), true)
            .await
            .0
            .is_client_error()
    );
    let mut invalid = set.clone();
    invalid["artifacts"]["tokenizer.json"]["sha256"] = json!("0".repeat(64));
    assert_eq!(at(app,"POST",route,Some(&json!({"specification":specification["id"],"artifact_set_bytes":invalid.to_string()})),true).await.0,StatusCode::BAD_REQUEST);
    let catalogue = at(
        app,
        "GET",
        "/api/webchat/v2/prefix-model-designs",
        None,
        true,
    )
    .await
    .1;
    assert_eq!(
        catalogue["artifact_sets"].as_array().unwrap(),
        &vec![stored]
    );
    assert_eq!(catalogue["design_generation_available"], false);
    let client = pool.get().await.unwrap();
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_prefix_model_artifact_bindings b JOIN reborn_prefix_model_specifications s USING(specification_id) WHERE s.provider_namespace=$1",
                &[&runtime.webui_tenant_id()]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert!(
        client
            .execute(
                "UPDATE reborn_prefix_model_artifact_sets SET inventory_bytes='[]'",
                &[]
            )
            .await
            .is_err(),
        "original model artifacts and inventory must be immutable"
    );
}

// Test-fixture preparation only; ALL provider triggers are restored in the same
// transaction before the real protected application path is exercised.
async fn exercise_prefix_revision_boundaries(
    app: &axum::Router,
    pool: &Arc<brassclaw_pg::PgPool>,
    runtime: &Arc<RebornRuntime>,
    generation_bytes: &str,
    temporary: &std::path::Path,
) {
    let provider = "prefix-model-acceptance";
    let tenant = runtime.webui_tenant_id();
    let capture_path = "/api/webchat/v2/prefix-model-specifications";
    let list_path = "/api/webchat/v2/prefix-model-designs";
    let generations_path = "/api/webchat/v2/prefixes/base-prompt/generations";
    let client = pool.get().await.unwrap();
    let old_history: Value = client.query_one("SELECT coalesce(jsonb_agg(to_jsonb(r) ORDER BY revision),'[]'::jsonb) FROM brassclaw_llm_provider_revisions r WHERE tenant_id=$1 AND provider_id=$2", &[&tenant,&provider]).await.unwrap().get(0);
    let jobs_before: i64 = client
        .query_one("SELECT count(*) FROM brassclaw_prefix_job_definitions", &[])
        .await
        .unwrap()
        .get(0);
    drop(client);
    let mut previous_capture = None;
    let mut previous_import = None;
    for (index, high) in [9_007_199_254_740_993_i64, i64::MAX]
        .into_iter()
        .enumerate()
    {
        let mut client = pool.get().await.unwrap();
        let tx = client.transaction().await.unwrap();
        tx.batch_execute("ALTER TABLE brassclaw_llm_providers DISABLE TRIGGER brassclaw_provider_revision_assignment").await.unwrap();
        assert_eq!(
            tx.execute(
                "UPDATE brassclaw_llm_providers SET revision=$3 WHERE tenant_id=$1 AND id=$2",
                &[&tenant, &provider, &high]
            )
            .await
            .unwrap(),
            1
        );
        tx.batch_execute("ALTER TABLE brassclaw_llm_providers ENABLE TRIGGER brassclaw_provider_revision_assignment").await.unwrap();
        tx.execute(
            "UPDATE reborn_prefix_profiles SET revision=$1 WHERE name='base-prompt'",
            &[&high],
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let triggers: bool = client.query_one("SELECT bool_and(tgenabled <> 'D') FROM pg_trigger WHERE tgrelid IN ('brassclaw_llm_providers'::regclass,'brassclaw_llm_provider_revisions'::regclass) AND NOT tgisinternal", &[]).await.unwrap().get(0);
        assert!(
            triggers,
            "every provider/history trigger is enabled before HTTP"
        );
        assert_eq!(client.query_one("SELECT definition FROM brassclaw_llm_provider_revisions WHERE tenant_id=$1 AND provider_id=$2 AND revision=$3", &[&tenant,&provider,&high]).await.unwrap().get::<_,Value>(0), client.query_one("SELECT definition FROM brassclaw_llm_providers WHERE tenant_id=$1 AND id=$2", &[&tenant,&provider]).await.unwrap().get::<_,Value>(0), "real recording trigger retained exact high original");
        drop(client);
        let (status, catalogue) = at(app, "GET", list_path, None, true).await;
        assert_eq!(status, StatusCode::OK);
        let target = catalogue["targets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["provider_id"] == provider)
            .unwrap();
        assert_eq!(target["provider_revision"], high.to_string());
        let capture = json!({"provider_id":target["provider_id"],"model":target["model"],"expected_provider_revision":target["provider_revision"]});
        assert_eq!(
            at(app, "POST", capture_path, Some(&capture), false).await.0,
            StatusCode::UNAUTHORIZED
        );
        let (status, captured) = at(app, "POST", capture_path, Some(&capture), true).await;
        assert_eq!(status, StatusCode::OK, "{captured}");
        assert_eq!(captured["provider_revision"], high.to_string());
        let inner: Value =
            serde_json::from_str(captured["specification_bytes"].as_str().unwrap()).unwrap();
        assert_eq!(
            inner["provider_revision"].as_i64(),
            Some(high),
            "retained inner JSON remains exact native integer"
        );
        assert_eq!(
            at(app, "POST", capture_path, Some(&capture), true).await.1,
            captured
        );
        let mut stale = capture.clone();
        stale["expected_provider_revision"] = json!((high - 1).to_string());
        assert_eq!(
            at(app, "POST", capture_path, Some(&stale), true).await.0,
            StatusCode::CONFLICT
        );
        if let Some((request, original)) = &previous_capture {
            assert_eq!(
                at(app, "POST", capture_path, Some(request), true).await.1,
                *original,
                "historical original capture survives later provider revision"
            );
        }
        // Valid compiler package, new OUTER identity only. Inner manifest bytes
        // and old immutable rows remain unchanged.
        let native_bytes = format!("{}{}\n", "\n".repeat(index + 1), generation_bytes);
        let import =
            json!({"expected_profile_revision":high.to_string(),"generation_bytes":native_bytes});
        assert_eq!(
            at(app, "POST", generations_path, Some(&import), false)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        let (status, retained) = at(app, "POST", generations_path, Some(&import), true).await;
        assert_eq!(status, StatusCode::OK, "{retained}");
        assert_eq!(retained["summary"]["profile_revision"], high.to_string());
        assert_eq!(retained["generation_bytes"], native_bytes);
        assert_eq!(retained["summary"]["qualification"], "quarantined");
        assert_eq!(
            at(app, "POST", generations_path, Some(&import), true)
                .await
                .1,
            retained
        );
        let mut stale = import.clone();
        stale["expected_profile_revision"] = json!((high - 1).to_string());
        assert_eq!(
            at(app, "POST", generations_path, Some(&stale), true)
                .await
                .0,
            StatusCode::CONFLICT
        );
        if let Some((request, original)) = &previous_import {
            assert_eq!(
                at(app, "POST", generations_path, Some(request), true)
                    .await
                    .1,
                *original,
                "historical generation retry never rebases to new profile revision"
            );
        }
        // Decode original revision on real route, but deliberately reject missing
        // probe prerequisites. This is NOT a successful measurement qualification.
        let measure = json!({"expected_profile_revision":high.to_string(),"generation":retained["summary"]["generation"],"proposal":"0".repeat(64),"artifact_set":"0".repeat(64),"probes_json":"[]","context_window_tokens":1,"reserved_additional_input_tokens":0});
        let job = Uuid::new_v4();
        let job_path = format!("/api/webchat/v2/prefixes/base-prompt/jobs/{job}");
        assert_eq!(
            at(app, "POST", &job_path, Some(&measure), false).await.0,
            StatusCode::UNAUTHORIZED
        );
        for _ in 0..2 {
            assert_eq!(
                at(app, "POST", &job_path, Some(&measure), true).await.0,
                StatusCode::BAD_REQUEST
            );
        }
        assert_eq!(
            at(app, "GET", &job_path, None, true).await.0,
            StatusCode::NOT_FOUND
        );
        let browser_bytes = format!(" \n{}\n", native_bytes);
        let browser_file = temporary.join(format!("revision-{high}.json"));
        std::fs::write(&browser_file, &browser_bytes).unwrap();
        let context = json!({"provider_id":provider,"revision":high.to_string()});
        if history_http_tests::exercise_revisions(app, &browser_file, &context).await {
            let (status, list) = at(app, "GET", generations_path, None, true).await;
            assert_eq!(status, StatusCode::OK);
            let id = brassclaw_model_encoding::checksum(browser_bytes.as_bytes());
            let row = list
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["generation"] == id)
                .expect("real browser import persisted its exact new package");
            assert_eq!(row["profile_revision"], high.to_string());
            let detail = at(app, "GET", &format!("{generations_path}/{id}"), None, true)
                .await
                .1;
            assert_eq!(detail["generation_bytes"], browser_bytes);
        }
        let client = pool.get().await.unwrap();
        assert_eq!(
            client
                .query_one("SELECT count(*) FROM brassclaw_prefix_job_definitions", &[])
                .await
                .unwrap()
                .get::<_, i64>(0),
            jobs_before,
            "rejected precision measurement creates no job"
        );
        drop(client);
        previous_capture = Some((capture, captured));
        previous_import = Some((import, retained));
    }
    // u64::MAX is valid wire data, but cannot name a PostgreSQL BIGINT
    // revision. Reject it before retaining anything; never narrow or round it.
    for (path, request) in [
        (
            capture_path,
            json!({"provider_id":provider,"model":"configured-inactive-model","expected_provider_revision":u64::MAX.to_string()}),
        ),
        (
            generations_path,
            json!({"expected_profile_revision":u64::MAX.to_string(),"generation_bytes":generation_bytes}),
        ),
    ] {
        assert_eq!(
            at(app, "POST", path, Some(&request), true).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    // Actual native backend contract remains integer even while browser/profile
    // revisions are high; original low-revision generations retain their pins.
    {
        use brassclaw_host_runtime::PrefixGenerationBackend;
        let backend =
            crate::pg_prefix_generation_backend::PgPrefixGenerationBackend::new(pool.clone());
        let scope = brassclaw_host_api::ResourceScope::system();
        let receipt = backend
            .retain("base-prompt", 1, generation_bytes, &scope)
            .await
            .unwrap();
        assert_eq!(receipt["profile_revision"].as_u64(), Some(1));
        let detail = backend
            .read(
                "base-prompt",
                receipt["generation"].as_str().unwrap(),
                &scope,
            )
            .await
            .unwrap();
        assert_eq!(detail["summary"]["profile_revision"].as_u64(), Some(1));
        assert_eq!(detail["generation_bytes"], generation_bytes);
    }
    let client = pool.get().await.unwrap();
    let unchanged: Value=client.query_one("SELECT coalesce(jsonb_agg(to_jsonb(r) ORDER BY revision),'[]'::jsonb) FROM brassclaw_llm_provider_revisions r WHERE tenant_id=$1 AND provider_id=$2 AND revision<$3", &[&tenant,&provider,&9_007_199_254_740_993_i64]).await.unwrap().get(0);
    assert_eq!(
        unchanged, old_history,
        "old provider history bytes were never rewritten"
    );
    println!(
        "native-prefix-revision-http-pg: passed; measurement rejected/no-job, not successful measurement"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_generation_qualification_preserves_configured_slots_and_original_executor() {
    use brassclaw_product_workflow::MontyVmSettingsStore;
    let rig = super::super::super::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let settings = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
        rig.pool.clone(),
        "default",
        "default",
    );
    let initial =
        brassclaw_product_workflow::MontyVmSettingsStore::get(&settings, "default", "default")
            .await
            .unwrap();
    let mut limits = initial.execution_limits;
    limits.max_source_bytes = 16 * 1024 * 1024;
    limits.max_compiled_source_bytes = limits
        .max_compiled_source_bytes
        .max(limits.max_source_bytes);
    limits.max_stdout_bytes = 16 * 1024 * 1024;
    limits.max_ipc_frame_bytes = 64 * 1024 * 1024;
    limits.max_workers = 2;
    brassclaw_product_workflow::MontyVmSettingsStore::upsert(
        &settings,
        "default",
        "default",
        &serde_json::from_value(json!({
            "expected_revision":initial.revision,"execution_limits":limits,
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        limits.utility_worker_memory_bytes,
        initial.execution_limits.utility_worker_memory_bytes
    );
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("generation-executor-owner", directory.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();
    let catalogue = &runtime.installed_catalogue;
    assert_eq!(catalogue.startup_executor.bounds().workers, 2);
    assert_eq!(
        catalogue.startup_executor.process().max_frame_bytes,
        64 * 1024 * 1024
    );
    // Also exercise the direct analogue without a fixture-sized frame/heap.
    crate::prefix_generation_behavior_tests::assert_native_generation_usages(catalogue).await;
    let client = rig.pool.get().await.unwrap();
    let changed_profile: i64 = client.query_one("UPDATE reborn_prefix_profiles SET label='Later operator profile',revision=revision+1 WHERE name='base-prompt' RETURNING revision", &[]).await.unwrap().get(0);
    assert!(changed_profile > 1);
    drop(client);
    let live = runtime.monty_settings_owner.store();
    let current = live.get("default", "default").await.unwrap();
    let mut reduced = current.execution_limits;
    reduced.max_source_bytes /= 2;
    reduced.max_stdout_bytes /= 2;
    live.upsert("default", "default", &serde_json::from_value(json!({
        "expected_revision":current.revision,"max_duration_secs":current.max_duration_secs+1,"execution_limits":reduced,
    })).unwrap()).await.unwrap();
    let acknowledged = live
        .effective_view()
        .get("default", "default")
        .await
        .unwrap();
    let smaller =
        crate::global_monty_startup::ControlledExecutorConfiguration::new(&acknowledged).unwrap();
    assert_ne!(
        smaller.exact_bytes(),
        catalogue.startup_executor.exact_bytes()
    );
    async fn counts(pool: &brassclaw_pg::PgPool) -> (i64, i64, i64) {
        let client = pool.get().await.unwrap();
        let row = client.query_one("SELECT (SELECT count(*) FROM brassclaw_monty_tool_invocations), (SELECT count(*) FROM brassclaw_monty_task_admissions), (SELECT count(*) FROM reborn_component_review_evidence)", &[]).await.unwrap();
        (row.get(0), row.get(1), row.get(2))
    }
    let before = counts(&rig.pool).await;
    // Hold the table's normal writer lock through a no-op INSERT of a genuine
    // other owner's exact immutable original. No fake evidence or row change.
    // The rejected global SHARE ROW EXCLUSIVE NOWAIT lock would fail here.
    let brassclaw_engine::executor::retained_recipe::RetainedProgram::Tools(reply) =
        catalogue.reply.program()
    else {
        panic!("actual qualified reply context required")
    };
    let reply_approval: Uuid = serde_json::from_value(
        reply
            .inputs()
            .instruction()
            .association_approvals()
            .values()
            .next()
            .unwrap()["approval_id"]
            .clone(),
    )
    .unwrap();
    let mut writer = rig.pool.get().await.unwrap();
    let approval_bytes: String = writer
        .query_one(
            "SELECT approval_bytes FROM reborn_skill_association_approvals WHERE approval_id=$1",
            &[&reply_approval],
        )
        .await
        .unwrap()
        .get(0);
    let approval: Value = serde_json::from_str(&approval_bytes).unwrap();
    let original_id: Uuid = serde_json::from_value(approval["q1_ref"].clone()).unwrap();
    let original_bytes: String = writer
        .query_one(
            "SELECT evidence_bytes FROM reborn_component_review_evidence WHERE evidence_id=$1",
            &[&original_id],
        )
        .await
        .unwrap()
        .get(0);
    let writer_tx = writer.transaction().await.unwrap();
    assert!(
        !crate::bootstrap_evidence::retain_intent(&writer_tx, original_id, &original_bytes)
            .await
            .unwrap()
    );
    let reused = crate::bootstrap_generation_approval::forbid_fresh_fixture(
        crate::bootstrap_generation_approval::qualify(
            rig.pool.clone(),
            catalogue.kernel.clone(),
            &catalogue._generation_candidates,
            &catalogue.worker,
            smaller.clone(),
        ),
    )
    .await
    .unwrap();
    writer_tx.rollback().await.unwrap();
    drop(writer);
    assert_eq!(reused.len(), catalogue.generation_approvals.len());
    for (old, read) in catalogue.generation_approvals.iter().zip(&reused) {
        assert_eq!(read.approval_id(), old.approval_id());
    }
    crate::bootstrap_generation_approval::assert_original_executor_custody(
        rig.pool.clone(),
        &reused,
        smaller.clone(),
    )
    .await;
    assert_eq!(
        counts(&rig.pool).await,
        before,
        "readback, malformed metadata and incomplete original guards must not replay effects or rewrite evidence"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_generation_unfinished_original_retains_fixture_after_profile_and_settings_edits() {
    use brassclaw_product_workflow::MontyVmSettingsStore;
    let rig = super::super::super::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let input = || {
        RebornRuntimeInput::from_services(
            rig.build_input("generation-intent-owner", directory.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
    };
    let interrupted = crate::bootstrap_generation_approval::interrupt_after_original_intents(
        Box::pin(build_reborn_runtime(input())),
    )
    .await;
    let error = match interrupted {
        Err(error) => error,
        Ok(_) => panic!("real original-intent boundary must interrupt startup"),
    };
    assert!(
        error
            .to_string()
            .contains("generation test interrupted after original intents")
    );
    async fn originals(pool: &brassclaw_pg::PgPool) -> Value {
        let client = pool.get().await.unwrap();
        client.query_one("SELECT coalesce(jsonb_agg(jsonb_build_object('id',evidence_id,'checksum',checksum,'fixture',evidence_bytes::jsonb #> '{report,fixture}') ORDER BY evidence_id),'[]'::jsonb) FROM reborn_component_review_evidence WHERE evidence_bytes::jsonb #>> '{kind}'='q1' AND evidence_bytes::jsonb #>> '{report,header,producer}'='brassclaw-prefix-generation-bootstrap/2'", &[]).await.unwrap().get(0)
    }
    async fn effects(pool: &brassclaw_pg::PgPool) -> (i64, i64) {
        let client = pool.get().await.unwrap();
        let row = client.query_one("SELECT (SELECT count(*) FROM brassclaw_monty_task_admissions WHERE scope->>'agent_id'='bootstrap-prefix-generation'),(SELECT count(*) FROM brassclaw_monty_tool_invocations i JOIN brassclaw_monty_task_admissions a USING(run_id) WHERE a.scope->>'agent_id'='bootstrap-prefix-generation')", &[]).await.unwrap();
        (row.get(0), row.get(1))
    }
    let original = originals(&rig.pool).await;
    assert_eq!(original.as_array().unwrap().len(), 2);
    assert_eq!(effects(&rig.pool).await, (0, 0));
    let settings = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
        rig.pool.clone(),
        "default",
        "default",
    );
    let current = settings.get("default", "default").await.unwrap();
    let mut changed = current.execution_limits;
    changed.max_source_bytes /= 2;
    changed.max_stdout_bytes /= 2;
    settings.upsert("default","default",&serde_json::from_value(json!({
        "expected_revision":current.revision,"max_duration_secs":current.max_duration_secs+1,"execution_limits":changed,
    })).unwrap()).await.unwrap();
    let client = rig.pool.get().await.unwrap();
    let revision: i64 = client.query_one("UPDATE reborn_prefix_profiles SET label='Later interrupted profile',revision=revision+1 WHERE name='base-prompt' RETURNING revision", &[]).await.unwrap().get(0);
    drop(client);
    assert!(revision as u64 > original[0]["fixture"]["profile_revision"].as_u64().unwrap());
    let resumed = crate::bootstrap_generation_approval::forbid_fresh_fixture(Box::pin(
        build_reborn_runtime(input()),
    ))
    .await;
    let error = match resumed {
        Err(error) => error,
        Ok(_) => panic!("an unresolved original is not a fresh fixture request"),
    };
    assert!(
        error
            .to_string()
            .contains("generation qualification intent is incomplete")
    );
    assert_eq!(
        originals(&rig.pool).await,
        original,
        "same immutable subject keeps its original fixture and IDs despite mutable edits"
    );
    assert_eq!(effects(&rig.pool).await, (0, 0));
}
