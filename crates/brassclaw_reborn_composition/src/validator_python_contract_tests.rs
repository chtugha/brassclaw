//! A closed contract-review Recipe, sequenced by the actual global Python flow.
//! The sealed observer verifies original subjects/results; no Q1 writer exists
//! here and this one-rule fixture is not the complete class-22 validator.
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use async_trait::async_trait;
use brassclaw_engine::{
    executor::{
        retained_recipe::{RetainedProgram, RetainedRecipeExecution},
        retained_source::InspectedRetainedProgram,
    },
    memory::{
        retained_inputs::prepare_retained_unbound_program,
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_validator::RetainedValidatorCoverage,
    },
};
use brassclaw_monty_host::{
    process::installed_worker,
    service::{PortFailure, TaskOutcome},
};
use brassclaw_skills::{
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS},
    global_bootstrap_components::packaged_component_id,
    review_submission_store::{
        PgReviewSubmissionStore, RetainedValidatorSubject, ReviewSubmissionDraft,
        selection::{self, RetainedValidatorSelection},
    },
    revision_store::PgComponentRevisionStore,
    validator_components::{
        aggregate_draft, collect_draft, python_contracts_draft, recursive_contract_draft,
    },
    value_contract::strict_json,
};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    bootstrap_recipe_flow, bootstrap_validation,
    global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe},
};

struct Invocation {
    reference: String,
    inspected: Arc<InspectedRetainedProgram>,
    subject: Arc<RetainedValidatorSubject>,
    saved: RetainedValidatorSelection,
    coverage: Arc<RetainedValidatorCoverage>,
    inputs: Value,
    report: Mutex<Option<Value>>,
    receipt_seen: AtomicBool,
}
fn invalid() -> PortFailure {
    PortFailure::new("contract_review_observation_invalid").unwrap()
}
#[async_trait]
impl InternalRecipeSource for Invocation {
    fn name(&self) -> &str {
        "development-python-contract-review"
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: self.inspected.clone(),
            inputs: Some(self.inputs.clone().into()),
            tools: None,
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        if !std::ptr::eq(selected, self.inspected.as_ref()) || !execution.host_answers().is_empty()
        {
            return Err(invalid());
        }
        let value = strict_json(receipt, REVISION_LIMITS).map_err(|_| invalid())?;
        let object = value
            .as_object()
            .filter(|o| o.len() == 2)
            .ok_or_else(invalid)?;
        if object.get("format") != Some(&json!("validator-step-results/1")) {
            return Err(invalid());
        }
        let results = object
            .get("results")
            .and_then(Value::as_object)
            .filter(|o| o.keys().map(String::as_str).eq(["0:1", "0:2", "0:3"]))
            .ok_or_else(invalid)?;
        let mut results: BTreeMap<String, Value> = results
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        results.insert("0:4".into(), json!(receipt));
        assert!(execution.is_complete());
        assert_eq!(
            execution.completed_step_ids().collect::<Vec<_>>(),
            ["0:1", "0:2", "0:3", "0:4"]
        );
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

fn candidate(id: Uuid, dependency: Uuid, valid: bool, marker: &str) -> ComponentRevisionDraft {
    let mut inputs = json!({"text":{"type":"string","required":true,"checks":[]}});
    if !valid {
        inputs["text"]["required"] = json!(false);
    }
    ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":id,"class_code":22,
        "document":{"name":marker,"content":"raise RuntimeError('candidate source stays data')\n",
            "input_contract":inputs,"result_contract":{"type":"string"},
            "preload":{"format":"python-preload/2","exports":{"candidate":{"symbol":"candidate","parameters":["inputs"],"mapping":true}},"private_functions":{},"constants":[],"imports":[],"dependencies":[dependency],"default_export":"candidate"}},
        "dependencies":[dependency],"association":null
    }).to_string()).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_python_contract_recipe_binds_complete_sealed_subject_and_rejects_same_output_substitution()
 {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    crate::builtin_bootstrap::retain_validator_check_drafts(&rig.pool)
        .await
        .unwrap();
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let helper =
        recursive_contract_draft(packaged_component_id("validator:recursive-contract")).unwrap();
    let rule = python_contracts_draft(
        packaged_component_id("validator:python-contracts"),
        helper.uuid(),
        crate::builtin_bootstrap::VALIDATOR_PYTHON_CONTRACTS_SEED,
    )
    .unwrap();
    for draft in [&helper, &rule] {
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
    let collector = collect_draft(Uuid::new_v4(), &["python_contracts".into()]).unwrap();
    let aggregate = aggregate_draft(Uuid::new_v4()).unwrap();
    let formatter=ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":Uuid::new_v4(),"class_code":22,
        "document":{"content":"def encode_results(inputs):\n    import json\n    return json.dumps({'format': 'validator-step-results/1', 'results': {'0:1': inputs['rule'], '0:2': inputs['checks'], '0:3': inputs['aggregation']}})\n",
            "input_contract":{
                "rule":with_required(rule.document()["result_contract"].clone()),
                "checks":with_required(collector.document()["result_contract"].clone()),
                "aggregation":with_required(aggregate.document()["result_contract"].clone())
            },"result_contract":{"type":"string"},
            "preload":{"format":"python-preload/2","exports":{"encode_results":{"symbol":"encode_results","parameters":["inputs"],"mapping":true}},"private_functions":{},"constants":[],"imports":["json"],"dependencies":[],"default_export":"encode_results"}},
        "dependencies":[],"association":null
    }).to_string()).unwrap();
    let recipe = Uuid::new_v4();
    let recipe_draft=ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":recipe,"class_code":21,
        "document":{"name":"development-contract-review","validates_class_code":22,"consumer_tags":["05:validator"],
            "validator_coverage":{"format":"validator-coverage/1","rules":[{"rule_id":"python_contracts","step_id":"0:1"}],"aggregate_step":"0:3"},
            "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":[],"variable_patterns":[]}],
            "step_descriptions":[{"desc_idx":0,"label":"Closed contract review only","yaml_source":"","steps":[
                {"stepnumber":1,"knowledge":"orchestrator","goal":"Check original public contracts","content":"","type":"component","include":[rule.uuid()]},
                {"stepnumber":2,"knowledge":"orchestrator","goal":"Collect complete actual result","content":"","type":"component","include":[collector.uuid()]},
                {"stepnumber":3,"knowledge":"orchestrator","goal":"Aggregate pinned required checks","content":"","type":"component","include":[aggregate.uuid()]},
                {"stepnumber":4,"knowledge":"orchestrator","goal":"Return results for actual observation verification","content":"","type":"component","include":[formatter.uuid()]}
            ]}],"input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":{"subject_json":{"type":"string","required":true,"checks":[]}},"steps":{
                "0:1":{"subject_json":{"kind":"task_input","reference":"{{vars.subject_json}}"}},
                "0:2":{"python_contracts":{"kind":"result","step_id":"0:1","path":[]}},
                "0:3":{"required_rule_ids":{"kind":"constant","value":["python_contracts"]},"checks":{"kind":"result","step_id":"0:2","path":[]}},
                "0:4":{"rule":{"kind":"result","step_id":"0:1","path":[]},"checks":{"kind":"result","step_id":"0:2","path":[]},"aggregation":{"kind":"result","step_id":"0:3","path":[]}}
            }}}},"dependencies":[rule.uuid(),collector.uuid(),aggregate.uuid(),formatter.uuid()],"association":null
    }).to_string()).unwrap();
    let mut references = Vec::new();
    for draft in [&helper, &rule] {
        references.push(
            store
                .read_revision(draft.uuid(), 1)
                .await
                .unwrap()
                .reference(),
        );
    }
    for draft in [&collector, &aggregate, &formatter, &recipe_draft] {
        references.push(store.stage(draft, 0).await.unwrap());
    }
    let graph = Arc::new(store.read_exact(&[recipe], &references).await.unwrap());
    let program = prepare_retained_unbound_program(
        compile_retained_recipe(graph, recipe, "selected", WorkflowClass::Deterministic).unwrap(),
    )
    .unwrap();
    let worker = installed_worker().unwrap();
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect(RetainedProgram::Unbound(Arc::new(program)), &worker)
            .await
            .unwrap(),
    );
    let pure = inspected.pure_component(rule.uuid()).unwrap();
    assert_eq!(pure.components().len(), 2);
    let coverage =
        Arc::new(RetainedValidatorCoverage::from_inputs(inspected.program().inputs()).unwrap());
    let private=ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":Uuid::new_v4(),"class_code":22,
        "document":{"content":"def private_helper(value):\n    return value\n","preload":{"format":"python-preload/2","exports":{},"private_functions":{"private_helper":["value"]},"constants":[],"imports":[],"dependencies":[],"default_export":null}},
        "dependencies":[],"association":null
    }).to_string()).unwrap();
    let private_ref = store.stage(&private, 0).await.unwrap();
    let mut invalid_private: Value = serde_json::from_str(private.exact_bytes()).unwrap();
    let invalid_private_id = Uuid::new_v4();
    invalid_private["uuid"] = json!(invalid_private_id);
    invalid_private["document"]["result_contract"] =
        json!({"type":"string","default":"outputs cannot have defaults"});
    let invalid_private = ComponentRevisionDraft::from_json(&invalid_private.to_string()).unwrap();
    let invalid_private_ref = store.stage(&invalid_private, 0).await.unwrap();
    let mut subjects = Vec::new();
    for (valid, dependency, marker) in [
        (true, private_ref, "original"),
        (false, private_ref, "invalid contract"),
        (
            true,
            private_ref,
            "different original with equal rule output",
        ),
        (
            true,
            invalid_private_ref,
            "invalid private dependency contract",
        ),
    ] {
        let submission = Uuid::new_v4();
        PgReviewSubmissionStore::new(rig.pool.clone())
            .submit(
                &ReviewSubmissionDraft::new(
                    submission,
                    "development-python-contract-review",
                    candidate(Uuid::new_v4(), dependency.uuid, valid, marker),
                    None,
                    vec![dependency],
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
        let bytes =
            RetainedValidatorCoverage::selection_bytes(&subject, &inspected, REVISION_LIMITS)
                .unwrap();
        selection::retain_in_transaction(&tx, &bytes, REVISION_LIMITS)
            .await
            .unwrap();
        let saved = selection::read_preparation_in_transaction(&tx, submission, REVISION_LIMITS)
            .await
            .unwrap()
            .unwrap();
        tx.commit().await.unwrap();
        subjects.push((subject, saved));
    }
    let root = crate::global_root_seed::retain_packaged_global_root(
        rig.pool.clone(),
        crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
    )
    .await
    .unwrap();
    let first_inputs = coverage
        .subject_task_inputs(
            &subjects[0].0,
            inspected.program().inputs(),
            REVISION_LIMITS,
        )
        .unwrap();
    for (index, (subject, saved)) in subjects.into_iter().enumerate() {
        let original_inputs = coverage
            .subject_task_inputs(&subject, inspected.program().inputs(), REVISION_LIMITS)
            .unwrap();
        let inputs = if index == 2 {
            first_inputs.clone()
        } else {
            original_inputs
        };
        let reference = format!("development-python-contract-review:{}", Uuid::new_v4());
        let admitted = Box::pin(bootstrap_validation::admit(
            rig.pool.clone(),
            "python-contract-review",
            &reference,
        ))
        .await
        .unwrap();
        let source = Arc::new(Invocation {
            reference,
            inspected: inspected.clone(),
            subject: subject.clone(),
            saved,
            coverage: coverage.clone(),
            inputs,
            report: Mutex::new(None),
            receipt_seen: AtomicBool::new(false),
        });
        let observed = bootstrap_recipe_flow::execute(
            &admitted,
            bootstrap_recipe_flow::Request::contained(
                worker.clone(),
                inspected.preparation_limits(),
                &root,
                source.clone(),
            )
            .unwrap(),
        )
        .await;
        assert!(
            source.receipt_seen.load(Ordering::SeqCst),
            "case {index}: receipt must reach the actual sealed observer"
        );
        if index == 2 {
            assert!(
                matches!(observed, Err(crate::RebornBuildError::InvalidConfig { ref reason }) if reason.contains("bootstrap Recipe completion: InvalidPortResult"))
            );
            assert!(source.report.lock().await.is_none());
            bootstrap_validation::settle_closed(&admitted)
                .await
                .unwrap();
            continue;
        }
        let observed = observed.unwrap_or_else(|error| panic!("case {index}: {error:?}"));
        assert!(
            matches!(
                observed.receipt.outcome,
                TaskOutcome::InternalCompleted { .. }
            ),
            "{:?}",
            observed.receipt.outcome
        );
        assert_eq!(observed.report["model_exchange_count"], 0);
        assert!(observed.report["reply"].is_null());
        let report = source.report.lock().await.clone().unwrap();
        assert_eq!(report["submission_id"], json!(subject.submission_id()));
        assert_eq!(report["q1_qualification"], false);
        assert_eq!(report["semantic_approval"], false);
        assert_eq!(report["aggregation"]["pass"], index == 0);
        observed
            .ports
            .observe_internal_execution(|execution| {
                assert!(execution.is_complete());
                assert!(execution.host_answers().is_empty());
                assert_eq!(
                    execution.completed_step_ids().collect::<Vec<_>>(),
                    ["0:1", "0:2", "0:3", "0:4"]
                );
            })
            .await
            .unwrap();
        bootstrap_validation::settle_closed(&admitted)
            .await
            .unwrap();
    }
    assert_eq!(
        rig.pool
            .get()
            .await
            .unwrap()
            .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[],)
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert_eq!(
        rig.pool
            .get()
            .await
            .unwrap()
            .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

fn with_required(mut value: Value) -> Value {
    value["required"] = json!(true);
    value
}
