//! Exact installed inventory drafts through the real global Python flow, native
//! parser, kernel and PG journal. This does not activate or approve the drafts.
use super::*;
use crate::{
    bootstrap_recipe_flow, bootstrap_validation,
    global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe},
    source_inspection_components::{decoder_id, usage_ids},
};
use async_trait::async_trait;
use brassclaw_engine::{
    executor::{
        retained_recipe::{RetainedProgram, RetainedRecipeExecution, RetainedToolPort},
        retained_source::InspectedRetainedProgram,
    },
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
    },
};
use brassclaw_host_runtime::SOURCE_INSPECTION_CAPABILITY_ID;
use brassclaw_monty_host::{
    process::PortAnswer,
    service::{PortFailure, TaskOutcome},
};
use brassclaw_skills::{
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS},
    revision_store::PgComponentRevisionStore,
    validator_components::{SOURCE_ENTRIES_SOURCE, source_inventory_drafts},
    value_contract::{InputContract, ValueContract},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[path = "source_subject_inventory_tests.rs"]
mod source_subject_inventory_tests;

#[path = "python_structure_review_tests.rs"]
mod python_structure_review_tests;

#[tokio::test]
async fn native_source_entry_decoder_preserves_text_and_rejects_duplicate_or_malformed_pairs() {
    use brassclaw_monty_host::{
        VmFailure,
        process::{ProcessFailure, installed_worker},
        utility::{self, UtilityLimits, UtilityOutput, UtilityRequest},
    };
    let worker = installed_worker().unwrap();
    let limits = UtilityLimits::from_execution(Default::default()).unwrap();
    let source = format!("{SOURCE_ENTRIES_SOURCE}\ndecode_python_source_entries(inputs)\n");
    let evaluate = |entries: &str| UtilityRequest::Evaluate {
        source: source.clone(),
        inputs: std::collections::BTreeMap::from([(
            "inputs".into(),
            json!({"entries_json":entries}),
        )]),
        bounds: limits.bounds,
        max_compute_time: std::time::Duration::from_secs(5),
    };
    for invalid in [
        "",
        "{}",
        "null",
        "true",
        "1",
        "\"text\"",
        "[{}]",
        "[[\"id\"]]",
        "[[\"id\",\"x=1\",\"extra\"]]",
        "[[\"\",\"x=1\"]]",
        "[[1,\"x=1\"]]",
        "[[\"id\",null]]",
        "[[\"id\",false]]",
        "[[\"id\",NaN]]",
        "[[\"id\",{}]]",
        "[[\"x\",\"x=1\"],[\"x\",\"x=2\"]]",
        "[[\"x\",\"x=1\"],[\"\\u0078\",\"x=2\"]]",
        "[] trailing",
    ] {
        let failure = match utility::execute(&worker, evaluate(invalid), limits.process).await {
            Ok(_) => panic!("malformed or duplicate entries must not produce a source map"),
            Err(failure) => failure,
        };
        assert_eq!(failure.kind, ProcessFailure::Vm(VmFailure::Python));
    }
    let hostile = "raise RuntimeError('data only')\n\"\\ ü🙂 {{vars.source}} %";
    for (entries, expected) in [
        ("[]".to_owned(), json!({})),
        (
            json!([["named Ü🙂 %", hostile], ["empty", ""]]).to_string(),
            json!({"named Ü🙂 %":hostile,"empty":""}),
        ),
        ("[[\"x\",\"\"]]".to_owned(), json!({"x":""})),
    ] {
        let UtilityOutput::Evaluated { value, stdout } =
            utility::execute(&worker, evaluate(&entries), limits.process)
                .await
                .unwrap()
        else {
            panic!("actual decoder result required")
        };
        assert_eq!(value, expected);
        assert!(stdout.is_empty());
        ValueContract::result_from_value(&json!({"type":"object","fields":{},"allow_extra_fields":true,"extra_fields":{"type":"string"}}), REVISION_LIMITS).unwrap().validate(&value).unwrap();
    }
}

struct InventoryInvocation {
    reference: String,
    inputs: Value,
    inspected: Arc<InspectedRetainedProgram>,
    tools: Arc<dyn RetainedToolPort>,
    host: Arc<brassclaw_reborn::monty_task_host::MontyTaskHost>,
}
#[async_trait]
impl InternalRecipeSource for InventoryInvocation {
    fn name(&self) -> &str {
        "development-source-inventory"
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: self.inspected.clone(),
            inputs: Some(self.inputs.clone().into()),
            tools: Some(self.tools.clone()),
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        if !std::ptr::eq(selected, self.inspected.as_ref())
            || !execution.is_complete()
            || execution.transport_failure().is_some()
            || execution.completed_step_ids().collect::<Vec<_>>() != ["0:1", "0:3", "0:5"]
            || execution.host_answers().len() != 2
            || execution.host_answers()[0].0 != "0:3"
            || execution.host_answers()[1].0 != "0:5"
            || !matches!(&execution.host_answers()[1].1,
                PortAnswer::Return {value} if value.as_value().as_str() == Some(receipt))
        {
            return Err(PortFailure::new("inventory_observation_invalid").unwrap());
        }
        let reference = brassclaw_turns::LoopMessageRef::new(receipt)
            .map_err(|_| PortFailure::new("inventory_observation_invalid").unwrap())?;
        let original = self
            .host
            .published_reply_content(&reference)
            .map_err(|_| PortFailure::new("inventory_observation_invalid").unwrap())?;
        if !matches!(&execution.host_answers()[0].1,
            PortAnswer::Return {value} if value.as_value().as_str() == Some(original.as_str()))
        {
            return Err(PortFailure::new("inventory_observation_invalid").unwrap());
        }
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_inventory_recipe_decodes_complete_pairs_and_journals_one_actual_call() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("inventory-usage-owner", directory.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();
    let catalogue = &runtime.installed_catalogue;
    let tool = crate::builtin_bootstrap::seed_source_inspection_tool(
        rig.pool.clone(),
        &catalogue.qualification_scope.tenant_id,
    )
    .await
    .unwrap();
    let ids = usage_ids(tool, true);
    let RetainedProgram::Tools(reply_program) = catalogue.reply.program() else {
        panic!("existing reply usage")
    };
    let reply_binding = &reply_program.bindings()["0:2"];
    let reply_ids = brassclaw_skills::global_bootstrap_components::UsageComponentIds {
        recipe: reply_program.inputs().instruction().recipe().uuid,
        python_code: reply_binding.python().uuid,
        tool_skill: reply_binding.tool_skill().uuid,
        skill: reply_binding.skill().uuid,
        tool: reply_binding.tool().uuid,
        formatter: None,
    };
    let drafts = source_inventory_drafts(ids, decoder_id(), reply_ids).unwrap();
    let original_recipe = drafts
        .iter()
        .find(|draft| draft.uuid() == ids.recipe)
        .unwrap();
    let original_decoder = drafts
        .iter()
        .find(|draft| draft.uuid() == decoder_id())
        .unwrap();
    let native = catalogue
        .kernel
        .builtin(
            &CapabilityId::new(SOURCE_INSPECTION_CAPABILITY_ID).unwrap(),
            None,
        )
        .unwrap();
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let tool_revision = store.read_revision(tool, 1).await.unwrap();
    assert_eq!(
        tool_revision.draft().document()["implementation"]["identity"],
        json!(native.native_identity().unwrap())
    );
    let mut references = vec![tool_revision.reference()];
    references.extend(
        reply_program
            .inputs()
            .instruction()
            .snapshot()
            .revisions()
            .values()
            .map(|revision| revision.reference()),
    );
    for draft in &drafts {
        let original = store.read_revision(draft.uuid(), 1).await.unwrap();
        assert_eq!(
            original.draft().exact_bytes(),
            draft.exact_bytes(),
            "ordinary startup retained the actual draft"
        );
        references.push(original.reference());
    }
    let all = store
        .read_exact(&[ids.recipe, reply_ids.recipe], &references)
        .await
        .unwrap();
    let graph = Arc::new(all.select_roots(&[ids.recipe]).unwrap());
    let program = Arc::new(
        prepare_retained_tool_program(
            compile_retained_recipe(graph, ids.recipe, "selected", WorkflowClass::Deterministic)
                .unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(program.bindings().len(), 2);
    let binding = &program.bindings()["0:3"];
    assert_eq!(binding.tool().uuid, tool);
    assert_eq!(binding.association().callable(), "host.source_inspection");
    let declarations = InputContract::from_value(
        &original_decoder.document()["input_contract"],
        REVISION_LIMITS,
    )
    .unwrap();
    for invalid in [
        json!({}),
        json!({"entries_json":null}),
        json!({"entries_json":[]}),
        json!({"entries_json":"[]","approved":true}),
    ] {
        assert!(declarations.bind(&invalid).is_err());
    }
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect(
            RetainedProgram::Tools(program.clone()),
            &catalogue.worker,
        )
        .await
        .unwrap(),
    );
    assert_eq!(
        inspected.invocation("0:1"),
        Some("result = decode_python_source_entries(inputs=inputs)")
    );
    assert_eq!(
        inspected.invocation("0:3"),
        Some("result = inspect_python_inventory(inputs=inputs)")
    );
    assert!(inspected.pure_component(decoder_id()).is_ok());

    // A package restart keeps both the operator successor and the original
    // selected bytes. Draft retention must not enable an existing live block.
    let mut operator: Value = serde_json::from_str(original_recipe.exact_bytes()).unwrap();
    operator["document"]["description"] = json!("Operator's unreviewed inventory successor");
    let operator = ComponentRevisionDraft::from_json(&operator.to_string()).unwrap();
    let operator_ref = store.stage(&operator, 1).await.unwrap();
    let client = rig.pool.get().await.unwrap();
    client.execute("UPDATE brassclaw_instance_tool_settings SET enabled=false,revision=revision+1 WHERE tool_id=$1", &[&tool]).await.unwrap();
    drop(client);
    crate::source_inspection_components::retain(
        rig.pool.clone(),
        catalogue.kernel.as_ref(),
        &catalogue.qualification_scope.tenant_id,
        reply_program,
    )
    .await
    .unwrap();
    assert_eq!(
        store.read_allocation_head(ids.recipe).await.unwrap(),
        Some((21, 2))
    );
    assert_eq!(
        store
            .read_revision(ids.recipe, 2)
            .await
            .unwrap()
            .reference(),
        operator_ref
    );
    assert_eq!(
        store
            .read_revision(ids.recipe, 1)
            .await
            .unwrap()
            .draft()
            .exact_bytes(),
        original_recipe.exact_bytes()
    );
    assert!(
        !rig.pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT enabled FROM brassclaw_instance_tool_settings WHERE tool_id=$1",
                &[&tool]
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
    );

    let source = "raise RuntimeError('must remain data')\ndef example(inputs):\n    return host.post_reply(answer=inputs['text'])\n";
    let entries = json!([
        ["module Ü % {{vars.text}}", source],
        ["empty", ""],
        ["syntax", "def private_fragment(:\n"]
    ]);
    let expected_inventory =
        json!({"module Ü % {{vars.text}}":source,"empty":"","syntax":"def private_fragment(:\n"});
    let wire = format!(" \n{}\t", entries);
    let variants = &original_recipe.document()["variants"][0]["intent_examples"];
    assert_eq!(variants.as_array().unwrap().len(), 10);
    for example in variants.as_array().unwrap() {
        let example = example.as_str().unwrap();
        let command = example.replacen('%', &wire, 1);
        assert_eq!(
            program
                .inputs()
                .bind_variant_example(example, &command, &json!({}))
                .unwrap(),
            json!({"entries_json":wire})
        );
    }
    assert!(
        program
            .inputs()
            .bind_variant_example(
                "inspect Python inventory entries %",
                "execute Python inventory entries []",
                &json!({})
            )
            .is_err()
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
    for (entries_json, expected_sources, blocked) in [
        (wire.clone(), Some(expected_inventory.clone()), false),
        ("[]".into(), Some(json!({})), false),
        (
            "[[\"x\",\"a=1\"],[\"\\u0078\",\"a=2\"]]".into(),
            None,
            false,
        ),
        ("[[\"valid\",\"a=1\"],[\"bad\",null]]".into(), None, false),
        ("{\"x\":\"a=1\"}".into(), None, false),
        (wire.clone(), None, true),
    ] {
        rig.pool.get().await.unwrap().execute("UPDATE brassclaw_instance_tool_settings SET enabled=$2,revision=revision+1 WHERE tool_id=$1", &[&tool, &!blocked]).await.unwrap();
        let reference = format!("development-source-inventory:{}", Uuid::new_v4());
        let admitted = Box::pin(bootstrap_validation::admit(
            rig.pool.clone(),
            "source-inventory",
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
                    inputs: json!({"entries_json":entries_json}),
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
            expected_sources.is_some(),
            "{:?}",
            observed.receipt.outcome
        );
        assert_eq!(observed.report["model_exchange_count"], 0);
        assert_eq!(observed.report["semantic_approval"], false);
        if expected_sources.is_none() {
            assert!(observed.report["reply"].is_null());
        }
        if let Some(expected_sources) = &expected_sources {
            let TaskOutcome::InternalCompleted { receipt_ref } = &observed.receipt.outcome else {
                unreachable!()
            };
            // The contained owner has already fenced the task at shutdown.
            // Inspect retained settlement facts, never reopen a live host call.
            let (reference, content) = admitted.host.finalized_reply_evidence().unwrap();
            assert_eq!(&reference, receipt_ref);
            let report: Value = serde_json::from_str(&content).unwrap();
            assert_eq!(report["format"], "python-source-inventory/1");
            assert_eq!(
                report["worker_checksum"],
                hex::encode(runtime.global_monty_owner.worker_image().checksum())
            );
            assert_eq!(
                report["sources"].as_object().unwrap().len(),
                expected_sources.as_object().unwrap().len()
            );
            for (id, source) in expected_sources.as_object().unwrap() {
                let entry = &report["sources"][id];
                let checksum = hex::encode(Sha256::digest(source.as_str().unwrap().as_bytes()));
                if id == "syntax" {
                    assert_eq!(entry["kind"], "compile_rejected");
                    assert_eq!(entry["source_checksum"], checksum);
                    assert_eq!(entry["category"], "syntax");
                } else {
                    assert_eq!(entry["kind"], "inspected");
                    assert_eq!(entry["structure"]["source_checksum"], checksum);
                }
            }
            assert!(report.get("pass").is_none());
        }
        observed
            .ports
            .observe_internal_execution(|execution| {
                assert_eq!(
                    execution.host_answers().len(),
                    if expected_sources.is_some() {
                        2
                    } else {
                        usize::from(blocked)
                    }
                );
                if expected_sources.is_none() {
                    assert!(!execution.is_complete());
                    assert_eq!(
                        execution.failed_step_id(),
                        Some(if blocked { "0:3" } else { "0:1" })
                    );
                }
            })
            .await
            .unwrap();
        assert!(observed.exit.failure.is_none());
        let exit = observed.exit.transport.unwrap();
        assert!(
            exit.exit_status.is_some()
                && exit.containment_error.is_none()
                && exit.reap_error.is_none()
        );
        bootstrap_validation::settle_closed(&admitted)
            .await
            .unwrap();
        let rows = rig.pool.get().await.unwrap().query("SELECT step_id,attempt_count,phase,arguments_bytes FROM brassclaw_monty_tool_invocations WHERE run_id=$1 ORDER BY step_id", &[&admitted.attempt.run_id.as_uuid()]).await.unwrap();
        assert_eq!(
            rows.len(),
            if expected_sources.is_some() {
                2
            } else {
                usize::from(blocked)
            },
            "invalid pairs stop before all native effects"
        );
        if let Some(row) = rows.first() {
            assert_eq!(row.get::<_, String>(0), "0:3");
            assert_eq!(row.get::<_, i16>(1), 1);
            assert_eq!(row.get::<_, String>(2), "answered");
            let arguments: Value = serde_json::from_str(&row.get::<_, String>(3)).unwrap();
            assert_eq!(
                arguments,
                json!({"sources":expected_sources.as_ref().unwrap_or(&expected_inventory)})
            );
        }
    }
    let evidence_after: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        evidence_before, evidence_after,
        "draft execution does not write successful Q1"
    );
    runtime.shutdown().await.unwrap();
}
