//! The original sealed subject is data for Python extraction, never candidate
//! execution. This closed flow does not qualify a class validator or activate it.
use super::*;
use brassclaw_engine::executor::retained_recipe::{
    typed_object_checksum_with_limits, typed_value_checksum_with_limits,
};
use brassclaw_skills::{
    global_bootstrap_components::packaged_component_id,
    review_submission_store::{PgReviewSubmissionStore, ReviewSubmissionDraft},
    validator_components::subject_sources_draft,
};

fn candidate(id: Uuid, dependencies: &[Uuid], content: &str) -> ComponentRevisionDraft {
    ComponentRevisionDraft::from_json(
        &json!({
            "format":"component-revision/1","uuid":id,"class_code":22,
            "document":{"content":content},"dependencies":dependencies,"association":null
        })
        .to_string(),
    )
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_sealed_subject_extracts_every_python_source_into_one_actual_inventory_call() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("subject-inventory-owner", directory.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();
    let catalogue = &runtime.installed_catalogue;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let extractor =
        subject_sources_draft(packaged_component_id("validator:subject-sources")).unwrap();
    let retained = store.read_revision(extractor.uuid(), 1).await.unwrap();
    assert_eq!(retained.draft().exact_bytes(), extractor.exact_bytes());
    let mut successor: Value = serde_json::from_str(extractor.exact_bytes()).unwrap();
    successor["document"]["description"] = json!("Operator's retained extractor successor");
    let successor = ComponentRevisionDraft::from_json(&successor.to_string()).unwrap();
    let successor_ref = store.stage(&successor, 1).await.unwrap();
    crate::builtin_bootstrap::retain_validator_check_drafts(&rig.pool)
        .await
        .unwrap();
    assert_eq!(
        store.read_allocation_head(extractor.uuid()).await.unwrap(),
        Some((22, 2))
    );
    assert_eq!(
        store
            .read_revision(extractor.uuid(), 2)
            .await
            .unwrap()
            .reference(),
        successor_ref
    );

    let root_id = Uuid::new_v4();
    let helper_id = Uuid::new_v4();
    let empty_id = Uuid::new_v4();
    let metadata_id = Uuid::new_v4();
    let old_id = Uuid::new_v4();
    let helper_text = "def helper(value):\n    return 'Ü🙂 \\\" {{vars.text}} %' + value\n";
    let root_text = "raise RuntimeError('candidate must never execute')\ndef run(inputs):\n    return host.post_reply(answer=inputs['text'])\n";
    let helper_ref = store
        .stage(&candidate(helper_id, &[], helper_text), 0)
        .await
        .unwrap();
    let empty_ref = store.stage(&candidate(empty_id, &[], ""), 0).await.unwrap();
    let metadata = ComponentRevisionDraft::from_json(
        &json!({
            "format":"component-revision/1","uuid":metadata_id,"class_code":13,
            "document":{"content":"This is binding metadata, not Python source"},
            "dependencies":[],"association":null
        })
        .to_string(),
    )
    .unwrap();
    let metadata_ref = store.stage(&metadata, 0).await.unwrap();
    store
        .stage(
            &candidate(old_id, &[], "raise RuntimeError('old dependency')"),
            0,
        )
        .await
        .unwrap();
    let base = store
        .stage(
            &candidate(root_id, &[old_id], "raise RuntimeError('comparison base')"),
            0,
        )
        .await
        .unwrap();
    let submission = Uuid::new_v4();
    PgReviewSubmissionStore::new(rig.pool.clone())
        .submit(
            &ReviewSubmissionDraft::new(
                submission,
                "development-subject-inventory",
                candidate(root_id, &[helper_id, empty_id, metadata_id], root_text),
                Some(base),
                vec![helper_ref, empty_ref, metadata_ref],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    // A newer dependency must not replace the review's exact original choice.
    store
        .stage(
            &candidate(helper_id, &[], "raise RuntimeError('unselected successor')"),
            1,
        )
        .await
        .unwrap();
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let subject = PgReviewSubmissionStore::read_validator_subject_in_transaction(
        &tx,
        submission,
        REVISION_LIMITS,
    )
    .await
    .unwrap()
    .unwrap();
    tx.commit().await.unwrap();
    drop(client);
    assert_eq!(subject.candidate().version, 2);
    assert_eq!(subject.value()["dependencies"].as_array().unwrap().len(), 3);
    assert_eq!(
        subject.value()["base_comparison"]["reference"]["version"],
        1
    );
    let expected_sources = json!({root_id.to_string():root_text,helper_id.to_string():helper_text,empty_id.to_string():""});

    let tool = crate::builtin_bootstrap::seed_source_inspection_tool(
        rig.pool.clone(),
        &catalogue.qualification_scope.tenant_id,
    )
    .await
    .unwrap();
    let inventory = usage_ids(tool, true);
    let RetainedProgram::Tools(reply) = catalogue.reply.program() else {
        panic!("existing reply usage")
    };
    let binding = &reply.bindings()["0:2"];
    let reply_ids = brassclaw_skills::global_bootstrap_components::UsageComponentIds {
        recipe: reply.inputs().instruction().recipe().uuid,
        python_code: binding.python().uuid,
        tool_skill: binding.tool_skill().uuid,
        skill: binding.skill().uuid,
        tool: binding.tool().uuid,
        formatter: None,
    };
    let drafts = source_inventory_drafts(inventory, decoder_id(), reply_ids).unwrap();
    let canonical = drafts
        .iter()
        .find(|draft| draft.uuid() == inventory.recipe)
        .unwrap();
    // A closed development Recipe reuses the exact installed usage/reply. Its
    // first pure component consumes the original subject instead of a command.
    let recipe_id = Uuid::new_v4();
    let mut recipe: Value = serde_json::from_str(canonical.exact_bytes()).unwrap();
    recipe["uuid"] = json!(recipe_id);
    recipe["document"]["variants"][0]["intent_examples"] = json!([]);
    recipe["document"]["variants"][0]["variable_patterns"] = json!([]);
    recipe["document"]["step_descriptions"][0]["steps"][0]["include"] = json!([extractor.uuid()]);
    recipe["document"]["input_layouts"]["selected"]["task_inputs"] =
        extractor.document()["input_contract"].clone();
    recipe["document"]["input_layouts"]["selected"]["steps"]["0:1"] =
        json!({"subject_json":{"kind":"task_input","reference":"{{vars.subject_json}}"}});
    recipe["document"]["invocations"]["selected"]["0:1"] = json!("extract_review_python_sources");
    for dependency in recipe["dependencies"].as_array_mut().unwrap() {
        if *dependency == json!(decoder_id()) {
            *dependency = json!(extractor.uuid());
        }
    }
    let recipe = ComponentRevisionDraft::from_json(&recipe.to_string()).unwrap();
    let recipe_ref = store.stage(&recipe, 0).await.unwrap();
    let mut references = vec![
        recipe_ref,
        retained.reference(),
        store.read_revision(tool, 1).await.unwrap().reference(),
    ];
    references.extend(
        reply
            .inputs()
            .instruction()
            .snapshot()
            .revisions()
            .values()
            .map(|revision| revision.reference()),
    );
    for draft in drafts
        .iter()
        .filter(|draft| draft.uuid() != inventory.recipe && draft.uuid() != decoder_id())
    {
        references.push(
            store
                .read_revision(draft.uuid(), 1)
                .await
                .unwrap()
                .reference(),
        );
    }
    let graph = store
        .read_exact(&[recipe_id, reply_ids.recipe], &references)
        .await
        .unwrap();
    let graph = Arc::new(graph.select_roots(&[recipe_id]).unwrap());
    let program = Arc::new(
        prepare_retained_tool_program(
            compile_retained_recipe(graph, recipe_id, "selected", WorkflowClass::Deterministic)
                .unwrap(),
        )
        .unwrap(),
    );
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect(
            RetainedProgram::Tools(program.clone()),
            &catalogue.worker,
        )
        .await
        .unwrap(),
    );
    assert!(inspected.pure_component(extractor.uuid()).is_ok());
    assert_eq!(
        inspected.invocation("0:1"),
        Some("result = extract_review_python_sources(inputs=inputs)")
    );
    let root = crate::global_root_seed::retain_packaged_global_root(
        rig.pool.clone(),
        crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
    )
    .await
    .unwrap();
    let evidence_before: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
        .await
        .unwrap()
        .get(0);
    let original = subject.value().clone();
    let mut duplicate = original.clone();
    duplicate["dependencies"]
        .as_array_mut()
        .unwrap()
        .push(original["candidate"].clone());
    let mut null_source = original.clone();
    null_source["candidate"]["revision"]["document"]["content"] = Value::Null;
    let mut missing_source = original.clone();
    // Choose a known Python dependency, independently of store ordering.
    for entry in missing_source["dependencies"].as_array_mut().unwrap() {
        if entry["reference"]["uuid"] == json!(helper_id) {
            entry["revision"]["document"]
                .as_object_mut()
                .unwrap()
                .remove("content");
        }
    }
    let mut wrong_identity = original.clone();
    wrong_identity["candidate"]["revision"]["uuid"] = json!(Uuid::new_v4());
    let mut boolean_class = original.clone();
    boolean_class["candidate"]["reference"]["class_code"] = json!(true);
    let mut extra_field = original.clone();
    extra_field["approved"] = json!(true);
    for (input, succeeds) in [
        (original.clone(), true),
        (duplicate, false),
        (null_source, false),
        (missing_source, false),
        (wrong_identity, false),
        (boolean_class, false),
        (extra_field, false),
        (original, true),
    ] {
        let task_inputs = json!({"subject_json":input.to_string()});
        let reference = format!("development-subject-inventory:{}", Uuid::new_v4());
        let admitted = Box::pin(bootstrap_validation::admit(
            rig.pool.clone(),
            "subject-inventory",
            &reference,
        ))
        .await
        .unwrap();
        let tools = crate::installed_monty_catalogue::source_inventory_candidate_tools(
            rig.pool.clone(),
            catalogue.kernel.clone(),
            admitted.host.clone(),
            admitted.admission.clone(),
            program.clone(),
        )
        .unwrap();
        let observed = bootstrap_recipe_flow::execute(
            &admitted,
            bootstrap_recipe_flow::Request::contained(
                catalogue.worker.clone(),
                inspected.preparation_limits(),
                &root,
                Arc::new(InventoryInvocation {
                    reference,
                    inputs: task_inputs.clone(),
                    inspected: inspected.clone(),
                    tools,
                    host: admitted.host.clone(),
                }),
            )
            .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(
            matches!(
                observed.receipt.outcome,
                TaskOutcome::InternalCompleted { .. }
            ),
            succeeds,
            "{:?}",
            observed.receipt.outcome
        );
        assert_eq!(observed.report["model_exchange_count"], 0);
        observed
            .ports
            .observe_internal_execution(|execution| {
                let observation = &execution.observations()["0:1"];
                assert_eq!(
                    observation.input_checksum(),
                    typed_object_checksum_with_limits(
                        &std::collections::BTreeMap::from([(
                            "subject_json".into(),
                            &task_inputs["subject_json"]
                        )]),
                        observation.data_limits(),
                    )
                    .unwrap()
                );
                if succeeds {
                    assert_eq!(
                        observation.result_checksum(),
                        Some(
                            typed_value_checksum_with_limits(
                                &expected_sources,
                                observation.data_limits()
                            )
                            .unwrap()
                        )
                    );
                    assert!(execution.is_complete());
                } else {
                    assert_eq!(execution.failed_step_id(), Some("0:1"));
                    assert!(execution.host_answers().is_empty());
                }
            })
            .await
            .unwrap();
        if succeeds {
            let (_, content) = admitted.host.finalized_reply_evidence().unwrap();
            let report: Value = serde_json::from_str(&content).unwrap();
            let sources = report["sources"].as_object().unwrap();
            assert_eq!(sources.len(), 3);
            assert!(!sources.contains_key(&old_id.to_string()));
            assert!(!sources.contains_key(&metadata_id.to_string()));
            for (id, text) in expected_sources.as_object().unwrap() {
                assert_eq!(sources[id]["kind"], "inspected");
                assert_eq!(
                    sources[id]["structure"]["source_checksum"],
                    hex::encode(Sha256::digest(text.as_str().unwrap().as_bytes()))
                );
            }
            assert_eq!(
                report["worker_checksum"],
                hex::encode(runtime.global_monty_owner.worker_image().checksum())
            );
        } else {
            assert!(observed.report["reply"].is_null());
        }
        assert!(observed.exit.failure.is_none());
        bootstrap_validation::settle_closed(&admitted)
            .await
            .unwrap();
        let rows = rig.pool.get().await.unwrap().query(
            "SELECT step_id,attempt_count,arguments_bytes FROM brassclaw_monty_tool_invocations WHERE run_id=$1 ORDER BY step_id",
            &[&admitted.attempt.run_id.as_uuid()],
        ).await.unwrap();
        assert_eq!(rows.len(), if succeeds { 2 } else { 0 });
        if let Some(row) = rows.first() {
            assert_eq!(row.get::<_, String>(0), "0:3");
            assert_eq!(row.get::<_, i16>(1), 1);
            assert_eq!(
                serde_json::from_str::<Value>(&row.get::<_, String>(2)).unwrap(),
                json!({"sources":expected_sources})
            );
        }
    }
    assert_eq!(
        evidence_before,
        rig.pool
            .get()
            .await
            .unwrap()
            .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[],)
            .await
            .unwrap()
            .get::<_, i64>(0)
    );
    runtime.shutdown().await.unwrap();
}
