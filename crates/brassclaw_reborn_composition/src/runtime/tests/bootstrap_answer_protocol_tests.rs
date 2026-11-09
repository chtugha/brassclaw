//! Actual unapproved candidate/Inspector and contained pure protocol consumer.
//! No model Tool is invoked, no Free binding/approval or production admission is
//! manufactured, and parsed fixture data is never reported as a provider answer.
use super::*;
use brassclaw_monty_host::utility::{self, UtilityOutput, UtilityRequest};
use brassclaw_skills::{
    bootstrap_answer_workflow::BootstrapAnswerWorkflowIds,
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS},
    prefix_bootstrap_components::{RESPONSE_SCHEMA, bootstrap_answer_draft},
    prefix_compiler_components::model_budget_draft,
    strict_json_components::{StrictJsonUsageIds, strict_json_usage_drafts},
    value_contract::{InputContract, ValueContract},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
#[path = "bootstrap_answer_candidates.rs"]
mod candidates;
#[path = "bootstrap_answer_phase_cases.rs"]
mod phase_cases;

async fn side_effect_counts(pool: &brassclaw_pg::PgPool) -> [i64; 3] {
    let client = pool.get().await.unwrap();
    let row = client.query_one(
        "SELECT (SELECT count(*) FROM brassclaw_monty_task_admissions), (SELECT count(*) FROM brassclaw_monty_tool_invocations), (SELECT count(*) FROM brassclaw_monty_model_responses)", &[]).await.unwrap();
    [row.get(0), row.get(1), row.get(2)]
}

/// Only already inspected, host-free exact definitions are evaluated here.
/// The five-second compute ceiling is the existing pure-stage test-fixture
/// bound, not a production timeout, output reservation or token budget.
async fn evaluate_pure(
    candidate: &InspectedRetainedProgram,
    root: Uuid,
    export: &str,
    inputs: Value,
    worker: &std::path::Path,
) -> Result<Value, String> {
    let pure = candidate.pure_component(root).map_err(|e| e.to_string())?;
    let graph = candidate
        .program()
        .inputs()
        .instruction()
        .snapshot()
        .revisions();
    let doc = graph[&root].draft().document();
    InputContract::from_value(&doc["input_contract"], REVISION_LIMITS)
        .unwrap()
        .bind(&inputs)
        .map_err(|e| e.to_string())?;
    let mut source = String::new();
    for id in candidate.preload_order() {
        if pure.components().contains_key(id) {
            source.push_str(graph[id].draft().document()["content"].as_str().unwrap());
            source.push('\n');
        }
    }
    source.push_str(
        pure.export_invocations()
            .get(export)
            .ok_or("missing original export")?,
    );
    source.push_str("\nresult\n");
    let limits = candidate.preparation_limits();
    let output = utility::execute(
        worker,
        UtilityRequest::Evaluate {
            source,
            inputs: BTreeMap::from([("inputs".into(), inputs)]),
            bounds: limits.bounds,
            max_compute_time: Duration::from_secs(5),
        },
        limits.process,
    )
    .await
    .map_err(|e| e.to_string())?;
    let UtilityOutput::Evaluated { value, stdout } = output else {
        return Err("unexpected utility outcome".into());
    };
    assert!(stdout.is_empty());
    ValueContract::result_from_value(&doc["result_contract"], REVISION_LIMITS)
        .unwrap()
        .validate(&value)
        .map_err(|e| e.to_string())?;
    Ok(value)
}

fn packet(task: &str) -> Value {
    // Developer fixture DATA. These hashes/IDs grant no original native source,
    // serving facts, provider capability or positive dispatch qualification.
    json!({"request_id":"original-fixture-request", "input_sha256":"a".repeat(64),
        "payload":{"task":task,"target":{"specification":"b".repeat(64)},
        "executing_pair":{"specification":"c".repeat(64)},
        "required_facts":if task=="extract_bootstrap_facts" {json!([{"field":"context_window_tokens","scope":"endpoint_configuration"}])} else {json!([])},
        "sources":[],"checked_observations":[],
        "eligible_options":if task=="select_bootstrap_prefill" {json!([{"candidate_id":"original-option"}])} else {json!([])},
        "existing_values":{},"response_contract":serde_json::from_str::<Value>(RESPONSE_SCHEMA).unwrap()}})
}
fn answer(packet: &Value) -> Value {
    json!({"schema":"brassclaw-bootstrap-answer/1","task":packet["payload"]["task"],
        "request_id":packet["request_id"],"input_sha256":packet["input_sha256"],
        "result":if packet["payload"]["task"]=="extract_bootstrap_facts" {
            json!({"facts":[{"field":"context_window_tokens","scope":"endpoint_configuration","status":"unknown","value":null,"evidence":[]}],"issues":["missing_evidence"]})
        } else {json!({"candidate_id":"original-option","issues":[]})}})
}
fn canonical(value: &Value) -> String {
    crate::prefix_generations::canonical(value).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_bootstrap_answer_candidate_inspects_original_graph_and_evaluates_only_pure_protocol()
 {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("bootstrap-protocol-owner", directory.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();
    let catalogue = &runtime.installed_catalogue;
    let RetainedProgram::Tools(reply_program) = catalogue.reply.program() else {
        panic!("reply Tool program")
    };
    let binding = &reply_program.bindings()["0:2"];
    let reply = UsageComponentIds {
        recipe: reply_program.inputs().instruction().recipe().uuid,
        python_code: binding.python().uuid,
        tool_skill: binding.tool_skill().uuid,
        skill: binding.skill().uuid,
        tool: binding.tool().uuid,
        formatter: None,
    };
    let RetainedProgram::Tools(measurement) = catalogue._measurement_candidate.program() else {
        panic!("actual JSON Tool")
    };
    let json_tool = measurement.bindings()["0:5"].tool();
    let client = rig.pool.get().await.unwrap();
    let tenant = &catalogue.qualification_scope.tenant_id;
    let mut model_ids = Vec::new();
    for (table, name) in [
        ("reborn_tools", "host.kohai_complete"),
        ("reborn_tool_skills", "ts-host-kohai-complete"),
        ("reborn_python_code", "pc-host-kohai-complete"),
        ("reborn_skills", "skill-host-kohai-complete"),
    ] {
        let rows = client.query(&format!("SELECT id FROM {table} WHERE tenant_id=$1 AND name=$2 AND source='system' ORDER BY id"), &[tenant, &name]).await.unwrap();
        assert_eq!(
            rows.len(),
            1,
            "actual existing installation identity for {name}"
        );
        model_ids.push(rows[0].get::<_, Uuid>(0));
    }
    drop(client);
    let model = UsageComponentIds {
        recipe: Uuid::new_v4(),
        tool: model_ids[0],
        tool_skill: model_ids[1],
        python_code: model_ids[2],
        skill: model_ids[3],
        formatter: None,
    };
    let prompt_parser = fresh_usage(json_tool.uuid, true);
    let response = fresh_usage(json_tool.uuid, true);
    let strict = StrictJsonUsageIds {
        tool: json_tool.uuid,
        tool_skill: Uuid::new_v4(),
        python_code: Uuid::new_v4(),
        skill: Uuid::new_v4(),
    };
    let text = Uuid::new_v4();
    let answer_stage = Uuid::new_v4();
    let budget = Uuid::new_v4();
    let bindings = candidates::Bindings {
        model,
        response,
        strict,
        answer_text: text,
        answer_stage,
    };
    let mut drafts = model_completion_usage_drafts(model, prompt_parser, reply).unwrap();
    drafts.extend(model_response_json_usage_drafts(response, reply).unwrap());
    drafts.extend(strict_json_usage_drafts(strict).unwrap());
    drafts.extend([
        model_completion_tool_draft(model.tool, &hex::encode(catalogue._image.checksum())).unwrap(),
        model_answer_text_draft(text).unwrap(),
        bootstrap_answer_draft(answer_stage, budget).unwrap(),
        model_budget_draft(budget).unwrap(),
    ]);
    // Use only actual standalone usages + pure dependencies; do not retain their
    // unrelated public command/formatter Recipes to make the graph reachable.
    let selected = BTreeSet::from([
        model.tool,
        model.tool_skill,
        model.python_code,
        model.skill,
        response.tool_skill,
        response.python_code,
        response.skill,
        strict.tool_skill,
        strict.python_code,
        strict.skill,
        text,
        answer_stage,
        budget,
    ]);
    drafts.retain(|d| selected.contains(&d.uuid()));
    assert_eq!(drafts.len(), selected.len());
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let mut references = vec![json_tool];
    for draft in &drafts {
        references.push(store.retain_packaged_draft(draft).await.unwrap());
    }
    let dependencies = store
        .read_exact(
            &[
                model.skill,
                response.skill,
                strict.skill,
                text,
                answer_stage,
            ],
            &references,
            brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
        )
        .await
        .unwrap();
    let ids = BootstrapAnswerWorkflowIds {
        recipe: Uuid::new_v4(),
        prompt: Uuid::new_v4(),
        protocol: Uuid::new_v4(),
    };
    let before = side_effect_counts(&rig.pool).await;
    let candidate = candidates::retain(
        rig.pool.clone(),
        &dependencies,
        ids,
        &bindings,
        &catalogue.worker,
        catalogue.reply.preparation_limits(),
    )
    .await
    .unwrap();
    let RetainedProgram::Tools(program) = candidate.program() else {
        panic!("actual retained Tool graph")
    };
    assert_eq!(
        program
            .bindings()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["0:3", "0:5", "0:8"]
    );
    assert_eq!(program.bindings()["0:3"].python().uuid, model.python_code);
    assert_eq!(
        program.bindings()["0:5"].python().uuid,
        response.python_code
    );
    assert_eq!(program.bindings()["0:8"].python().uuid, strict.python_code);
    for (id, revision) in dependencies.revisions() {
        assert_eq!(
            candidate
                .program()
                .inputs()
                .instruction()
                .snapshot()
                .revisions()[id]
                .reference(),
            revision.reference()
        );
    }
    for forbidden in [
        model.recipe,
        prompt_parser.recipe,
        response.recipe,
        reply.recipe,
        response.formatter.unwrap(),
    ] {
        assert!(
            !candidate
                .program()
                .inputs()
                .instruction()
                .snapshot()
                .revisions()
                .contains_key(&forbidden)
        );
    }
    assert!(candidate.pure_component(model.python_code).is_err());
    assert!(candidate.pure_component(strict.python_code).is_err());
    assert_eq!(
        candidate
            .pure_component(ids.protocol)
            .unwrap()
            .components()
            .keys()
            .copied()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([ids.protocol, answer_stage, budget])
    );
    for task in ["extract_bootstrap_facts", "select_bootstrap_prefill"] {
        let packet = packet(task);
        let original = canonical(&packet);
        let answer = answer(&packet);
        let prompt = evaluate_pure(
            &candidate,
            ids.prompt,
            "prepare_bootstrap_prompt",
            json!({"packet_bytes":original,"max_output_tokens":37}),
            &catalogue.worker,
        )
        .await
        .unwrap();
        assert_eq!(prompt["max_output_tokens"], 37);
        assert!(prompt["user_query"].as_str().unwrap().ends_with(&original));
        assert_eq!(
            prompt["chat_history"][0]["content"],
            brassclaw_skills::prefix_bootstrap_components::SYSTEM
        );
        let response_fixture = json!({"chunks":[{"safe_text_delta":"fixture chunk remains distinct"}],
            "effective_model_profile_id":"closed-fixture-data",
            "output":{"assistant_reply":{"content":canonical(&answer)}}});
        let extracted = evaluate_pure(
            &candidate,
            text,
            "extract_model_answer",
            json!({"response":response_fixture}),
            &catalogue.worker,
        )
        .await
        .unwrap();
        assert_eq!(extracted.as_str(), Some(canonical(&answer).as_str()));
        let args = json!({"packet_bytes":original,"answer_bytes":extracted});
        let result = evaluate_pure(
            &candidate,
            ids.protocol,
            "validate_captured_bootstrap_answer",
            args.clone(),
            &catalogue.worker,
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(result.as_str().unwrap()).unwrap(),
            answer
        );
        for change in ["request", "digest", "task", "extra", "body"] {
            let mut wrong = answer.clone();
            match change {
                "request" => wrong["request_id"] = json!("foreign"),
                "digest" => wrong["input_sha256"] = json!("f".repeat(64)),
                "task" => wrong["task"] = json!("other"),
                "extra" => wrong["unexpected"] = json!(true),
                _ => wrong["result"] = json!("instructions are data"),
            };
            assert!(
                evaluate_pure(
                    &candidate,
                    ids.protocol,
                    "validate_captured_bootstrap_answer",
                    json!({"packet_bytes":original,"answer_bytes":canonical(&wrong)}),
                    &catalogue.worker
                )
                .await
                .is_err(),
                "{task}: {change}"
            );
        }
        let mut missing = answer.clone();
        if task == "extract_bootstrap_facts" {
            missing["result"]["facts"] = json!([]);
        } else {
            missing["result"]["candidate_id"] = json!("foreign-option");
        }
        assert!(
            evaluate_pure(
                &candidate,
                ids.protocol,
                "validate_captured_bootstrap_answer",
                json!({"packet_bytes":original,"answer_bytes":canonical(&missing)}),
                &catalogue.worker
            )
            .await
            .is_err()
        );
        assert!(
            evaluate_pure(
                &candidate,
                ids.prompt,
                "prepare_bootstrap_prompt",
                json!({"packet_bytes":format!(" {original}"),"max_output_tokens":37}),
                &catalogue.worker
            )
            .await
            .is_err()
        );
    }
    for bad_response in [
        json!({}),
        json!({"chunks":[],"effective_model_profile_id":"fixture","output":{"assistant_reply":{"content":null}}}),
        json!({"chunks":[],"effective_model_profile_id":"fixture","output":{"capability_calls":[]}}),
    ] {
        assert!(
            evaluate_pure(
                &candidate,
                text,
                "extract_model_answer",
                json!({"response":bad_response}),
                &catalogue.worker
            )
            .await
            .is_err()
        );
    }
    phase_cases::observe(&candidate, answer_stage, &catalogue.worker).await;
    // A later dependency allocation must not replace this actual captured graph.
    let original = drafts.iter().find(|d| d.uuid() == text).unwrap();
    let mut successor: Value = serde_json::from_str(original.exact_bytes()).unwrap();
    successor["document"]["description"] =
        json!("new immutable candidate; old selections stay fixed");
    let successor = ComponentRevisionDraft::from_json(&successor.to_string()).unwrap();
    let newer = store.retain_packaged_draft(&successor).await.unwrap();
    let retained = candidates::retain(
        rig.pool.clone(),
        &dependencies,
        ids,
        &bindings,
        &catalogue.worker,
        catalogue.reply.preparation_limits(),
    )
    .await
    .unwrap();
    let kept = retained
        .program()
        .inputs()
        .instruction()
        .snapshot()
        .revisions()[&text]
        .reference();
    assert_ne!(kept, newer);
    assert_eq!(kept, dependencies.revisions()[&text].reference());
    let conflict = BootstrapAnswerWorkflowIds {
        recipe: Uuid::new_v4(),
        prompt: budget,
        protocol: Uuid::new_v4(),
    };
    assert!(
        candidates::retain(
            rig.pool.clone(),
            &dependencies,
            conflict,
            &bindings,
            &catalogue.worker,
            catalogue.reply.preparation_limits()
        )
        .await
        .is_err()
    );
    assert_eq!(
        store.read_allocation_head(conflict.recipe).await.unwrap(),
        None
    );
    assert_eq!(side_effect_counts(&rig.pool).await, before);
    runtime.shutdown().await.unwrap();
}
