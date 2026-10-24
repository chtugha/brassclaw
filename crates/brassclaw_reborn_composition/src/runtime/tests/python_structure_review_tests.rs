//! Actual installed drafts/native inventory through a closed private Recipe.
//! Partial structural coverage does not create Q1, activation or pure approval.
use super::*;
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
};

use brassclaw_engine::memory::retained_validator::RetainedValidatorCoverage;
use brassclaw_skills::{
    global_bootstrap_components::packaged_component_id,
    review_submission_store::{
        PgReviewSubmissionStore, RetainedValidatorSubject, ReviewSubmissionDraft,
        selection::{self, RetainedValidatorSelection},
    },
    validator_components::python_structure_drafts,
    value_contract::strict_json,
};
use tokio::sync::Mutex;

struct StructureInvocation {
    reference: String,
    inspected: Arc<InspectedRetainedProgram>,
    tools: Arc<dyn RetainedToolPort>,
    subject: Arc<RetainedValidatorSubject>,
    saved: RetainedValidatorSelection,
    coverage: Arc<RetainedValidatorCoverage>,
    inputs: Value,
    worker_checksum: String,
    receipt_seen: AtomicBool,
    report: Mutex<Option<Value>>,
}
fn invalid() -> PortFailure {
    PortFailure::new("python_structure_observation_invalid").unwrap()
}
#[async_trait]
impl InternalRecipeSource for StructureInvocation {
    fn name(&self) -> &str {
        "development-python-structure-review"
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
            || execution.host_answers().len() != 1
            || execution.host_answers()[0].0 != "0:3"
        {
            return Err(invalid());
        }
        let value = strict_json(receipt, REVISION_LIMITS).map_err(|_| invalid())?;
        let object = value
            .as_object()
            .filter(|v| v.len() == 2)
            .ok_or_else(invalid)?;
        if object.get("format") != Some(&json!("validator-step-results/1")) {
            return Err(invalid());
        }
        let values = object
            .get("results")
            .and_then(Value::as_object)
            .filter(|v| {
                v.keys()
                    .map(String::as_str)
                    .eq(["0:1", "0:3", "0:4", "0:5", "0:6", "0:7"])
            })
            .ok_or_else(invalid)?;
        if !matches!(&execution.host_answers()[0].1,PortAnswer::Return {value}
            if value.as_value()==&values["0:3"])
        {
            return Err(invalid());
        }
        let inventory = strict_json(values["0:3"].as_str().ok_or_else(invalid)?, REVISION_LIMITS)
            .map_err(|_| invalid())?;
        if inventory["worker_checksum"] != self.worker_checksum {
            return Err(invalid());
        }
        let mut results: BTreeMap<_, _> = values
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        results.insert("0:8".into(), json!(receipt));
        assert!(!self.receipt_seen.swap(true, Ordering::SeqCst));
        let observed = self
            .coverage
            .observe_subject(
                &self.subject,
                &self.saved,
                execution,
                &results,
                REVISION_LIMITS,
            )
            .map_err(|_| invalid())?;
        *self.report.lock().await = Some(observed.report().clone());
        Ok(())
    }
}

fn python(id: Uuid, dependencies: &[Uuid], content: &str, public: bool) -> Value {
    let preload = if public {
        json!({"format":"python-preload/2","exports":{"candidate":{"symbol":"candidate","parameters":["inputs"],"mapping":true}},
            "private_functions":{},"constants":[],"imports":[],"dependencies":dependencies,"default_export":"candidate"})
    } else {
        json!({"format":"python-preload/2","exports":{},"private_functions":{"helper":["value"]},
            "constants":[],"imports":[],"dependencies":dependencies,"default_export":null})
    };
    let mut document = json!({"name":"review subject","content":content,"preload":preload});
    if public {
        document["input_contract"] = json!({"text":{"type":"string","required":true,"checks":[]}});
        document["result_contract"] = json!({"type":"string"});
    }
    json!({"format":"component-revision/1","uuid":id,"class_code":22,
        "document":document,"dependencies":dependencies,"association":null})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_private_structure_recipe_joins_original_inventory_definitions_contracts_and_sealed_observer()
 {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("python-structure-owner", directory.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();
    let catalogue = &runtime.installed_catalogue;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let tool = crate::builtin_bootstrap::seed_source_inspection_tool(
        rig.pool.clone(),
        &catalogue.qualification_scope.tenant_id,
    )
    .await
    .unwrap();
    let usage = usage_ids(tool, true);
    let drafts = python_structure_drafts(
        usage,
        crate::builtin_bootstrap::VALIDATOR_STRUCTURE_RECEIPT_SEED,
    )
    .unwrap();
    let recipe = drafts.iter().find(|d| d.class_code() == 21).unwrap();
    for draft in &drafts {
        assert_eq!(
            store
                .read_revision(draft.uuid(), 1)
                .await
                .unwrap()
                .draft()
                .exact_bytes(),
            draft.exact_bytes()
        );
    }
    let mut operator: Value = serde_json::from_str(recipe.exact_bytes()).unwrap();
    operator["document"]["description"] = json!("operator successor stays unreviewed");
    let operator = ComponentRevisionDraft::from_json(&operator.to_string()).unwrap();
    let successor = store.stage(&operator, 1).await.unwrap();
    crate::builtin_bootstrap::retain_validator_structure_drafts(&rig.pool, usage)
        .await
        .unwrap();
    assert_eq!(
        store.read_allocation_head(recipe.uuid()).await.unwrap(),
        Some((21, 2))
    );
    assert_eq!(
        store
            .read_revision(recipe.uuid(), 2)
            .await
            .unwrap()
            .reference(),
        successor
    );
    // Pin the actual packaged graph, including both checking libraries and the
    // registered inventory Tool. Never select the newer operator draft midway.
    let mut pending = vec![recipe.uuid()];
    let mut references = BTreeMap::new();
    while let Some(id) = pending.pop() {
        if references.contains_key(&id) {
            continue;
        }
        let revision = store.read_revision(id, 1).await.unwrap();
        pending.extend(revision.draft().dependencies());
        references.insert(id, revision.reference());
    }
    let graph = Arc::new(
        store
            .read_exact(
                &[recipe.uuid()],
                &references.values().copied().collect::<Vec<_>>(),
            )
            .await
            .unwrap(),
    );
    let program = Arc::new(
        prepare_retained_tool_program(
            compile_retained_recipe(
                graph,
                recipe.uuid(),
                "selected",
                WorkflowClass::Deterministic,
            )
            .unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(program.bindings().len(), 1);
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect(
            RetainedProgram::Tools(program.clone()),
            &catalogue.worker,
        )
        .await
        .unwrap(),
    );
    for id in ["validator:python-definitions", "validator:python-contracts"] {
        assert_eq!(
            inspected
                .pure_component(packaged_component_id(id))
                .unwrap()
                .components()
                .len(),
            2
        );
    }
    let coverage =
        Arc::new(RetainedValidatorCoverage::from_inputs(inspected.program().inputs()).unwrap());
    let root = crate::global_root_seed::retain_packaged_global_root(
        rig.pool.clone(),
        crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
    )
    .await
    .unwrap();
    let evidence_before = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
        .await
        .unwrap()
        .get::<_, i64>(0);
    let source = "def candidate(inputs):\n    return helper(inputs['text'])\n";
    let mut first_inputs = None;
    for case in 0..10 {
        let helper = Uuid::new_v4();
        let candidate = Uuid::new_v4();
        let mut helper_doc = python(helper, &[], "def helper(value):\n    return value\n", false);
        let mut candidate_doc = python(candidate, &[helper], source, true);
        match case {
            1 => {
                candidate_doc["document"]["preload"]["exports"]["candidate"]["parameters"] =
                    json!(["wrong"])
            }
            2 => {
                helper_doc["document"]["preload"]["private_functions"]["helper"] = json!(["other"])
            }
            3 => candidate_doc["document"]["preload"]["unexpected"] = json!(true),
            4 => candidate_doc["document"]["content"] = json!("def candidate(:\n"),
            5 => {
                candidate_doc["document"]["content"] = json!(format!(
                    "raise RuntimeError('candidate stays data')\n{source}"
                ))
            }
            6 => candidate_doc["document"]["input_contract"]["text"]["required"] = json!(false),
            7 => {
                helper_doc["document"]["result_contract"] =
                    json!({"type":"string","default":"invalid output default"})
            }
            8 => candidate_doc["document"]["preload"]["dependencies"] = json!([]),
            _ => {}
        }
        let helper_ref = store
            .stage(
                &ComponentRevisionDraft::from_json(&helper_doc.to_string()).unwrap(),
                0,
            )
            .await
            .unwrap();
        let submission = Uuid::new_v4();
        PgReviewSubmissionStore::new(rig.pool.clone())
            .submit(
                &ReviewSubmissionDraft::new(
                    submission,
                    "development-python-structure-review",
                    ComponentRevisionDraft::from_json(&candidate_doc.to_string()).unwrap(),
                    None,
                    vec![helper_ref],
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        let subject = Arc::new(
            PgReviewSubmissionStore::read_validator_subject_in_transaction(
                &tx,
                submission,
                REVISION_LIMITS,
            )
            .await
            .unwrap()
            .unwrap(),
        );
        let selection_bytes =
            RetainedValidatorCoverage::selection_bytes(&subject, &inspected, REVISION_LIMITS)
                .unwrap();
        selection::retain_in_transaction(&tx, &selection_bytes, REVISION_LIMITS)
            .await
            .unwrap();
        let saved = selection::read_preparation_in_transaction(&tx, submission, REVISION_LIMITS)
            .await
            .unwrap()
            .unwrap();
        tx.commit().await.unwrap();
        drop(client);
        let mut inputs = coverage
            .subject_task_inputs(&subject, inspected.program().inputs(), REVISION_LIMITS)
            .unwrap();
        if case == 0 {
            first_inputs = Some(inputs.clone());
        }
        if case == 9 {
            inputs = first_inputs.clone().unwrap();
        }
        let input_subject: Value =
            serde_json::from_str(inputs["subject_json"].as_str().unwrap()).unwrap();
        let mut expected_sources = serde_json::Map::new();
        for entry in std::iter::once(&input_subject["candidate"])
            .chain(input_subject["dependencies"].as_array().unwrap())
        {
            if entry["reference"]["class_code"] == 22 {
                expected_sources.insert(
                    entry["reference"]["uuid"].as_str().unwrap().to_owned(),
                    entry["revision"]["document"]["content"].clone(),
                );
            }
        }
        let reference = format!("development-python-structure:{}", Uuid::new_v4());
        let admitted = Box::pin(bootstrap_validation::admit(
            rig.pool.clone(),
            "python-structure-review",
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
        let invocation = Arc::new(StructureInvocation {
            reference,
            inspected: inspected.clone(),
            tools,
            subject,
            saved,
            coverage: coverage.clone(),
            inputs,
            worker_checksum: hex::encode(runtime.global_monty_owner.worker_image().checksum()),
            receipt_seen: AtomicBool::new(false),
            report: Mutex::new(None),
        });
        let observed = bootstrap_recipe_flow::execute(
            &admitted,
            bootstrap_recipe_flow::Request::contained(
                catalogue.worker.clone(),
                inspected.preparation_limits(),
                &root,
                invocation.clone(),
            )
            .unwrap(),
        )
        .await;
        if !invocation.receipt_seen.load(Ordering::SeqCst) {
            match &observed {
                Ok(observed) => {
                    let state = observed
                        .ports
                        .observe_internal_execution(|execution| {
                            format!(
                                "failed={:?}, completed={:?}, transport_failure={}",
                                execution.failed_step_id(),
                                execution.completed_step_ids().collect::<Vec<_>>(),
                                execution.transport_failure().is_some()
                            )
                        })
                        .await;
                    panic!(
                        "case {case}: native results did not reach observer: {:?}, {state:?}",
                        observed.receipt.outcome
                    );
                }
                Err(error) => {
                    panic!("case {case}: native results did not reach observer: {error:?}")
                }
            }
        }
        if case == 9 {
            assert!(
                matches!(observed,Err(crate::RebornBuildError::InvalidConfig {ref reason}) if reason.contains("bootstrap Recipe completion: InvalidPortResult"))
            );
            assert!(invocation.report.lock().await.is_none());
        } else {
            let observed = observed.unwrap_or_else(|error| panic!("case {case}: {error:?}"));
            assert!(matches!(
                observed.receipt.outcome,
                TaskOutcome::InternalCompleted { .. }
            ));
            assert_eq!(observed.report["model_exchange_count"], 0);
            assert!(observed.report["reply"].is_null());
            let report = invocation.report.lock().await.clone().unwrap();
            assert_eq!(report["q1_qualification"], false);
            assert_eq!(report["semantic_approval"], false);
            assert_eq!(
                report["aggregation"]["pass"],
                case == 0,
                "case {case}: {report}"
            );
            let rules = report["rules"].as_array().unwrap();
            assert_eq!(rules.len(), 2);
            assert_eq!(
                rules
                    .iter()
                    .find(|r| r["rule_id"] == "python_contracts")
                    .unwrap()["ok"],
                case != 6 && case != 7
            );
            assert_eq!(
                rules
                    .iter()
                    .find(|r| r["rule_id"] == "python_definitions")
                    .unwrap()["ok"],
                case == 0 || case == 6 || case == 7
            );
        }
        bootstrap_validation::settle_closed(&admitted)
            .await
            .unwrap();
        let rows=rig.pool.get().await.unwrap().query("SELECT step_id,attempt_count,arguments_bytes FROM brassclaw_monty_tool_invocations WHERE run_id=$1",&[&admitted.attempt.run_id.as_uuid()]).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get::<_, String>(0), "0:3");
        assert_eq!(rows[0].get::<_, i16>(1), 1);
        assert_eq!(
            serde_json::from_str::<Value>(&rows[0].get::<_, String>(2)).unwrap(),
            json!({"sources":expected_sources})
        );
    }
    assert_eq!(
        rig.pool
            .get()
            .await
            .unwrap()
            .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        evidence_before
    );
    runtime.shutdown().await.unwrap();
}
