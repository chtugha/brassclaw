//! Retained review submissions -> read-only preparation -> actual operator
//! publication -> ordinary global Monty. Reuses actual bundled usage approval.
use super::*;
use crate::runtime::RebornRuntime;
async fn at(
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
    let input = RebornRuntimeInput::from_services(
        rig.build_input("component-deployment-owner", root.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )
    .with_model_gateway_override(Arc::new(RecordingGateway {
        reply: "unmatched reply".into(),
        requests: Arc::new(StdMutex::new(Vec::new())),
    }))
    .with_poll_settings(PollSettings {
        interval: Duration::from_millis(10),
        // Real large source compilation is asynchronous; this acceptance
        // observer needs more than the ordinary ten-second chat fixture.
        max_total: Duration::from_secs(120),
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
    let client = rig.pool.get().await.unwrap();
    let dispatches: i64 = client
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get(0);
    drop(client);
    let reused = Box::pin(crate::bootstrap_generation_approval::qualify(
        rig.pool.clone(),
        catalogue.kernel.clone(),
        &catalogue._generation_candidates,
        &catalogue.worker,
    ))
    .await
    .unwrap();
    assert_eq!(reused.len(), 2);
    let compiler_approval = catalogue
        .compiler_approval
        .as_ref()
        .expect("closed compiler qualification before readiness");
    let reused_compiler = Box::pin(crate::bootstrap_compiler_approval::qualify(
        rig.pool.clone(),
        catalogue.kernel.clone(),
        &catalogue._source_compiler_candidates.source,
        &catalogue.generation_approvals,
        &catalogue.worker,
    ))
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        reused_compiler.approval_id(),
        compiler_approval.approval_id()
    );
    crate::bootstrap_compiler_approval::assert_incomplete_intent_is_not_replayed(
        rig.pool.clone(),
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
    let mut route_client = rig.pool.get().await.unwrap();
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
    let mut client = rig.pool.get().await.unwrap();
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
    let prefix_job_id =
        crate::runtime::prefix_job_submission::assert_global_snapshot_submission(&runtime).await;

    // The canonical Retain workflow may use only its own qualified formatter.
    // A different Recipe sharing the storage usage cannot borrow that coverage.
    for candidate in &catalogue._generation_candidates {
        let instruction = candidate.program().inputs().instruction();
        let selection: brassclaw_product_workflow::CatalogueRecipeSelection =
            serde_json::from_value(json!({"recipe_id":instruction.recipe().uuid,
                "workflow_class":"deterministic","components":instruction.snapshot().revisions().values()
                    .map(|r| reference(r.reference())).collect::<Vec<_>>(),"association_approvals":{}})).unwrap();
        let mut client = rig.pool.get().await.unwrap();
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
            brassclaw_skills::revision_store::PgComponentRevisionStore::new(rig.pool.clone())
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
        let mut client = rig.pool.get().await.unwrap();
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
    let large = std::process::Command::new("python3").arg("-c")
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
            prefix_metadata_llm_service(rig.pool.clone(), runtime.webui_tenant_id()).await,
        )
        .with_interceptor_config_service(Arc::new(
            crate::interceptor_config_service::RebornInterceptorConfigService::new(
                rig.pool.clone(),
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
        rig.pool
            .get()
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
                2
            } else {
                1
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
    let registry_client = rig.pool.get().await.unwrap();
    registry_client.execute("UPDATE reborn_prefix_profiles SET label='Operator label',revision=revision+1 WHERE name='sempai'", &[]).await.unwrap();
    drop(registry_client);
    brassclaw_pg::migrations::run_migrations(&rig.pool)
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
    assert_eq!(sempai["profile_revision"], 2);
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
        let output = std::process::Command::new("python3")
            .arg(&compiler)
            .args(args)
            .output()
            .unwrap();
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
            crate::pg_prefix_generation_backend::PgPrefixGenerationBackend::new(rig.pool.clone());
        let scope = brassclaw_host_api::ResourceScope::system();
        let receipt = backend
            .retain("base-prompt", 1, &generation_bytes, &scope)
            .await
            .unwrap();
        assert_eq!(receipt["qualification"], "quarantined");
        let reread = backend
            .read(
                "base-prompt",
                receipt["generation"].as_str().unwrap(),
                &scope,
            )
            .await
            .unwrap();
        assert_eq!(reread["generation_bytes"], generation_bytes);
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
    let storage_client = rig.pool.get().await.unwrap();
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
    let source_client = rig.pool.get().await.unwrap();
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
    exercise_prefix_model_records(&model_app, &rig.pool, &runtime, prefix_job_id).await;
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
    let client = rig.pool.get().await.unwrap();
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
    drop(app);
    drop(catalogue);
    shutdown_shared_runtime(runtime).await.unwrap();
}

async fn exercise_prefix_model_records(
    app: &axum::Router,
    pool: &Arc<brassclaw_pg::PgPool>,
    runtime: &Arc<RebornRuntime>,
    prefix_job_id: Uuid,
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
    let provider = json!({"id":"prefix-model-acceptance","adapter":"open_ai_completions","base_url":"http://127.0.0.1:1/v1","default_model":"exact-model","set_active":true});
    let (status, saved) = at(
        app,
        "POST",
        "/api/webchat/v2/llm/providers",
        Some(&provider),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
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
    exercise_prefix_model_artifacts(app, pool, &specification, runtime, prefix_job_id).await;
    let proposal_bytes = json!({"format":"brassclaw-prefix-design-proposal/1","profile":"sempai","specification":specification["id"],"design":{"adapter":"ornith-qwen-system/1","evidence_view":"literal-reference/v1","source_selection":"complete-mandatory","reserve_tokens":8192},"rationale":"Recorded proposal; no quality or architecture qualification asserted."}).to_string();
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
    let selection =
        json!({"provider_id":"prefix-model-acceptance","model":"different-model","role":"kohai"});
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
        let output = std::process::Command::new("python3")
            .arg(&compiler)
            .args(args)
            .output()
            .unwrap();
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
    let selected = store.read_exact(&[ids[0]], &refs).await.unwrap();
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
            .read_exact(&[ids[0]], &[successor])
            .await
            .unwrap()
            .revisions()
            .len(),
        1
    );
    let original = store.read_exact(&[ids[0]], &refs).await.unwrap();
    assert_eq!(original.revisions().len(), ids.len());
    assert_eq!(original.revisions()[&ids[0]].reference(), refs[0]);
    let mut cyclic = refs.clone();
    *cyclic.last_mut().unwrap() = store
        .stage(&draft(*ids.last().unwrap(), &[ids[0]]), 1)
        .await
        .unwrap();
    let error = match store.read_exact(&[ids[0]], &cyclic).await {
        Err(error) => error,
        Ok(_) => panic!("cyclic exact selection must fail"),
    };
    assert!(error.to_string().contains("cyclic dependency graph"));
    assert_eq!(
        store
            .read_exact(&[ids[0]], &refs)
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
