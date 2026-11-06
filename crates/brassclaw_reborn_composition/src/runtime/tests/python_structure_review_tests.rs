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
    validator_components::{python_pure_structure_drafts, python_structure_drafts},
    value_contract::strict_json,
};
use secrecy::ExposeSecret;
use tokio::sync::Mutex;

struct StructureInvocation {
    pure: bool,
    reference: String,
    inspected: Arc<InspectedRetainedProgram>,
    tools: Arc<dyn RetainedToolPort>,
    subject: Arc<RetainedValidatorSubject>,
    saved: Arc<RetainedValidatorSelection>,
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
                let expected: &[&str] = if self.pure {
                    &["0:1", "0:3", "0:4", "0:5", "0:6", "0:7", "0:8", "0:9"]
                } else {
                    &["0:1", "0:3", "0:4", "0:5", "0:6", "0:7", "0:8"]
                };
                v.keys().map(String::as_str).eq(expected.iter().copied())
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
        results.insert(
            if self.pure { "0:10" } else { "0:9" }.into(),
            json!(receipt),
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

// Negative probes reuse the actual completed execution. Only the untouched
// receipt can establish an observation; no substitute Tool returns are used.
struct ReceiptRejectionProbe {
    inner: Arc<crate::validator_inventory_review::InventoryReviewSource>,
    pure: bool,
    foreign: Arc<crate::validator_inventory_review::InventoryReviewSource>,
}
#[async_trait]
impl InternalRecipeSource for ReceiptRejectionProbe {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn reference(&self) -> &str {
        self.inner.reference()
    }
    fn captured_program(&self) -> Option<Arc<InspectedRetainedProgram>> {
        self.inner.captured_program()
    }
    fn requires_step_observations(&self) -> bool {
        true
    }
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        self.inner.select().await
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        let original: Value = serde_json::from_str(receipt).unwrap();
        let mut variants = vec![format!(
            "{{\"format\":\"validator-step-results/1\",{}",
            receipt.strip_prefix('{').unwrap()
        )];
        let mut unknown = original.clone();
        unknown["unknown"] = json!(true);
        variants.push(unknown.to_string());
        let mut missing = original.clone();
        missing["results"].as_object_mut().unwrap().remove("0:4");
        variants.push(missing.to_string());
        let mut extra = original.clone();
        extra["results"]["0:2"] = json!(null);
        variants.push(extra.to_string());
        let mut aggregate = original.clone();
        aggregate["results"][if self.pure { "0:9" } else { "0:8" }] =
            json!("not the actual aggregate object");
        variants.push(aggregate.to_string());
        let mut wrong_worker = original.clone();
        let mut inventory: Value =
            serde_json::from_str(original["results"]["0:3"].as_str().unwrap()).unwrap();
        inventory["worker_checksum"] = json!("00".repeat(32));
        wrong_worker["results"]["0:3"] = json!(inventory.to_string());
        variants.push(wrong_worker.to_string());
        let mut sources = original.clone();
        sources["results"]["0:1"] = json!({});
        variants.push(sources.to_string());
        for changed in variants {
            assert!(
                self.inner
                    .verify_receipt(&changed, selected, execution)
                    .await
                    .is_err()
            );
            assert!(self.inner.take_observation().await.is_none());
        }
        assert!(
            self.foreign
                .verify_receipt(receipt, selected, execution)
                .await
                .is_err(),
            "an answer from another original run cannot be borrowed"
        );
        assert!(self.foreign.take_observation().await.is_none());
        self.inner
            .verify_receipt(receipt, selected, execution)
            .await
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_private_structure_recipe_joins_original_inventory_definitions_contracts_and_sealed_observer()
 {
    check_private_structure_recipe(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_private_pure_structure_recipe_checks_all_exports_and_hidden_host_calls() {
    check_private_structure_recipe(true).await;
}

async fn check_private_structure_recipe(pure: bool) {
    use brassclaw_product_workflow::MontyVmSettingsStore;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    if !pure {
        let settings = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
            rig.pool.clone(),
            "default",
            "default",
        );
        let original = settings.get("default", "default").await.unwrap();
        let mut limits = original.execution_limits;
        limits.max_source_bytes = 16 * 1024 * 1024;
        limits.max_compiled_source_bytes = limits
            .max_compiled_source_bytes
            .max(limits.max_source_bytes);
        limits.max_stdout_bytes = 16 * 1024 * 1024;
        let changed = settings
            .upsert(
                "default",
                "default",
                &serde_json::from_value(json!({
                    "expected_revision":original.revision,"execution_limits":limits,
                }))
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            changed.execution_limits.utility_worker_memory_bytes,
            original.execution_limits.utility_worker_memory_bytes
        );
    }
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("python-structure-owner", directory.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();
    let catalogue = &runtime.installed_catalogue;
    assert_eq!(catalogue.startup_executor.bounds().workers, 2);
    if !pure {
        assert_eq!(
            catalogue.startup_executor.bounds().values.max_source_bytes,
            16 * 1024 * 1024
        );
        assert_eq!(
            catalogue.startup_executor.process().max_frame_bytes,
            64 * 1024 * 1024
        );
    }
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let tool = crate::builtin_bootstrap::seed_source_inspection_tool(
        rig.pool.clone(),
        &catalogue.qualification_scope.tenant_id,
    )
    .await
    .unwrap();
    let usage = usage_ids(tool, true);
    let profile =
        serde_json::to_value(brassclaw_engine::executor::retained_profile::profile()).unwrap();
    if pure {
        let conflicting = brassclaw_skills::global_bootstrap_components::UsageComponentIds {
            tool: packaged_component_id("validator:python-pure-structure:recipe"),
            ..usage
        };
        assert!(
            python_pure_structure_drafts(
                conflicting,
                crate::builtin_bootstrap::VALIDATOR_PURE_STRUCTURE_RECEIPT_SEED,
                profile.clone()
            )
            .is_err()
        );
    }
    let drafts = if pure {
        python_pure_structure_drafts(
            usage,
            crate::builtin_bootstrap::VALIDATOR_PURE_STRUCTURE_RECEIPT_SEED,
            profile,
        )
    } else {
        python_structure_drafts(
            usage,
            crate::builtin_bootstrap::VALIDATOR_STRUCTURE_RECEIPT_SEED,
            profile,
        )
    }
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
            3
        );
    }
    let names: &[&str] = if pure {
        &["validator:python-graph", "validator:python-pure-calls"]
    } else {
        &["validator:python-graph"]
    };
    for name in names {
        assert_eq!(
            inspected
                .pure_component(packaged_component_id(name))
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
    let RetainedProgram::Tools(reply) = catalogue.reply.program() else {
        panic!("retained reply factory context required")
    };
    let qualification_id = Uuid::new_v4();
    let qualified_parser = crate::validator_inventory_review::qualify(
        rig.pool.clone(),
        catalogue.kernel.clone(),
        catalogue.worker_image.clone(),
        &inspected,
        reply,
        qualification_id,
        catalogue.startup_executor.clone(),
        catalogue.resource_store_settings.clone(),
        catalogue.root_definition.clone(),
    )
    .await
    .unwrap();
    let qualification_calls_before = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get::<_, i64>(0);
    let readback_pool =
        Arc::new(brassclaw_pg::pool::build_pool_with_max_size(rig.url.expose_secret(), 1).unwrap());
    let settings = runtime.monty_settings_owner.store();
    let current = settings.get("default", "default").await.unwrap();
    let mut reduced = current.execution_limits;
    reduced.max_source_bytes /= 2;
    reduced.max_stdout_bytes /= 2;
    reduced.max_value_bytes /= 2;
    settings.upsert("default", "default", &serde_json::from_value(json!({
        "expected_revision":current.revision,"max_duration_secs":current.max_duration_secs+1,
        "execution_limits":reduced,
    })).unwrap()).await.unwrap();
    let changed = settings
        .effective_view()
        .get("default", "default")
        .await
        .unwrap();
    let next_executor =
        crate::global_monty_startup::ControlledExecutorConfiguration::new(&changed).unwrap();
    assert_ne!(
        next_executor.exact_bytes(),
        catalogue.startup_executor.exact_bytes()
    );
    tokio::time::timeout(std::time::Duration::from_secs(60),
        crate::validator_inventory_review::assert_original_installation_readback(
            catalogue,readback_pool.clone(),next_executor.clone(),
            brassclaw_monty_host::utility::UtilityLimits::from_execution(changed.execution_limits).unwrap(),
        )).await.expect("pool1 original installation metadata read must complete after acknowledged limit reductions");

    assert!(
        tokio::time::timeout(
            std::time::Duration::from_secs(60),
            crate::validator_inventory_review::qualify(
                readback_pool.clone(),
                catalogue.kernel.clone(),
                catalogue.worker_image.clone(),
                &inspected,
                reply,
                qualification_id,
                next_executor.clone(),
                catalogue.resource_store_settings.clone(),
                catalogue.root_definition.clone(),
            ),
        )
        .await
        .expect("completed original readback does not hold a client while Turn checks out pool1")
        .is_ok(),
        "a completed original qualification is read without replaying native usage"
    );
    readback_pool.close();
    assert_eq!(
        rig.pool
            .get()
            .await
            .unwrap()
            .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        qualification_calls_before
    );
    assert!(qualified_parser.require_program(reply).is_err());
    #[cfg(unix)]
    if !pure {
        crate::validator_inventory_review::assert_actual_ack_and_timeout_faults(
            catalogue,
            &inspected,
            reply,
            next_executor.clone(),
            rig.url.expose_secret(),
        )
        .await;
    }
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
    let case_count = if pure { 30 } else { 17 };
    for case in 0..case_count {
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
            10 => {
                candidate_doc["document"]["content"] =
                    json!("def candidate(inputs):\n    return missing(inputs['text'])\n")
            }
            11 => {
                candidate_doc["document"]["content"] =
                    json!("def candidate(inputs):\n    return inputs['text']\n");
                helper_doc["document"]["content"] =
                    json!("def candidate(value):\n    return value\n");
                helper_doc["document"]["preload"]["private_functions"] =
                    json!({"candidate":["value"]});
            }
            12 => {
                helper_doc["document"]["preload"]["exports"] =
                    json!({"candidate":{"symbol":"helper","parameters":["value"],"mapping":false}});
                helper_doc["document"]["preload"]["private_functions"] = json!({});
                helper_doc["document"]["preload"]["default_export"] = json!("candidate");
                helper_doc["document"]["input_contract"] =
                    json!({"value":{"type":"string","required":true,"checks":[]}});
                helper_doc["document"]["result_contract"] = json!({"type":"string"});
            }
            13 => {
                candidate_doc["document"]["preload"]["exports"]["candidate"]["mapping"] =
                    json!(false)
            }
            16 => {
                candidate_doc["document"]["content"] =
                    json!("def candidate(inputs):\n    return __execute_action__('not executed')\n")
            }
            17 => {
                candidate_doc["document"]["content"] = json!(
                    "def candidate(inputs):\n    return host.post_reply(text=inputs['text'])\n"
                )
            }
            18 => {
                candidate_doc["document"]["content"] =
                    json!("def candidate(inputs):\n    return inputs['text']\n");
                helper_doc["document"]["content"] =
                    json!("def helper(value):\n    return host.post_reply(text=value)\n");
            }
            19 => {
                candidate_doc["document"]["content"] =
                    json!("def candidate(inputs):\n    return candidate(inputs)\n")
            }
            20 | 23 => {
                helper_doc["document"]["content"] =
                    json!("def helper(value):\n    return helper(value)\n");
                if case == 23 {
                    candidate_doc["document"]["content"] =
                        json!("def candidate(inputs):\n    return inputs['text']\n");
                }
            }
            21 | 22 | 26 => {
                let extra = match case {
                    21 => "def another(inputs):\n    return another(inputs)\n",
                    22 => "def another(inputs):\n    return helper(inputs['text'])\n",
                    _ => "def another(inputs):\n    return host.post_reply(text=inputs['text'])\n",
                };
                candidate_doc["document"]["content"] = json!(format!("{source}{extra}"));
                candidate_doc["document"]["preload"]["exports"]["another"] =
                    json!({"symbol":"another","parameters":["inputs"],"mapping":true});
            }
            24 => {
                candidate_doc["document"]["content"] =
                    json!("VALUE = 1\ndef candidate(inputs):\n    return VALUE(inputs['text'])\n");
                candidate_doc["document"]["preload"]["constants"] = json!(["VALUE"]);
            }
            25 => {
                candidate_doc["document"]["content"] = json!(
                    "def candidate(inputs):\n    return ''.join(map(helper, inputs['text']))\n"
                )
            }
            27 => {
                helper_doc["document"]["content"] =
                    json!("def helper(value):\n    return list(map(helper, value))\n")
            }
            28 => {
                candidate_doc["document"]["preload"]["exports"] = json!({});
                candidate_doc["document"]["preload"]["private_functions"] =
                    json!({"candidate":["inputs"]});
                candidate_doc["document"]["preload"]["default_export"] = Value::Null;
            }
            29 => {
                candidate_doc["document"]["content"] =
                    json!("def candidate(inputs):\n    return 'host.post_reply(text=inputs)'\n")
            }
            _ => {}
        }
        let mut dependency_refs = Vec::new();
        if case == 14 || case == 15 {
            let other = Uuid::new_v4();
            let mut other_doc = python(other, &[], "def other(value):\n    return value\n", false);
            other_doc["document"]["preload"]["private_functions"] = json!({"other":["value"]});
            dependency_refs.push(
                store
                    .stage(
                        &ComponentRevisionDraft::from_json(&other_doc.to_string()).unwrap(),
                        0,
                    )
                    .await
                    .unwrap(),
            );
            helper_doc["document"]["content"] =
                json!("def helper(value):\n    return other(value)\n");
            if case == 14 {
                candidate_doc["dependencies"] = json!([helper, other]);
                candidate_doc["document"]["preload"]["dependencies"] = json!([helper, other]);
            } else {
                helper_doc["dependencies"] = json!([other]);
                helper_doc["document"]["preload"]["dependencies"] = json!([other]);
            }
        }
        let helper_ref = store
            .stage(
                &ComponentRevisionDraft::from_json(&helper_doc.to_string()).unwrap(),
                0,
            )
            .await
            .unwrap();
        dependency_refs.push(helper_ref);
        let submission = Uuid::new_v4();
        PgReviewSubmissionStore::new(rig.pool.clone())
            .submit(
                &ReviewSubmissionDraft::new(
                    submission,
                    "development-python-structure-review",
                    ComponentRevisionDraft::from_json(&candidate_doc.to_string()).unwrap(),
                    None,
                    dependency_refs,
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
        let saved = Arc::new(
            selection::read_preparation_in_transaction(&tx, submission, REVISION_LIMITS)
                .await
                .unwrap()
                .unwrap(),
        );
        tx.commit().await.unwrap();
        drop(client);
        let mut inputs = coverage
            .subject_task_inputs(&subject, inspected.program().inputs(), REVISION_LIMITS)
            .unwrap();
        if case == 0 {
            crate::pg_review_admission_source::assert_original_capture(
                catalogue,
                subject.clone(),
                saved.clone(),
                settings.as_ref(),
            )
            .await;
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
        let reference = crate::pg_review_admission_source::validator_review_reference(submission);
        let admitted = Box::pin(bootstrap_validation::admit(
            rig.pool.clone(),
            "python-structure-review",
            &reference,
            None,
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
        let mut foreign_admission = None;
        let mut foreign_source = None;
        let qualified = if case == 9 {
            None
        } else {
            let request = crate::validator_inventory_review::InventoryReviewRequest {
                pool: rig.pool.clone(),
                kernel: catalogue.kernel.clone(),
                host: admitted.host.clone(),
                admission: admitted.admission.clone(),
                subject: subject.clone(),
                saved: saved.clone(),
                inspected: inspected.clone(),
                parser: qualified_parser.clone(),
                profile: if pure {
                    crate::validator_inventory_review::InventoryReviewProfile::Pure
                } else {
                    crate::validator_inventory_review::InventoryReviewProfile::Structure
                },
            };
            if case == 0 {
                let mut wrong_profile = request.clone();
                wrong_profile.profile = if pure {
                    crate::validator_inventory_review::InventoryReviewProfile::Structure
                } else {
                    crate::validator_inventory_review::InventoryReviewProfile::Pure
                };
                assert!(
                    crate::validator_inventory_review::InventoryReviewSource::prepare(
                        wrong_profile
                    )
                    .await
                    .is_err()
                );
                let foreign = bootstrap_validation::admit(
                    rig.pool.clone(),
                    "foreign-inventory-review",
                    &reference,
                    None,
                )
                .await
                .unwrap();
                let mut wrong_run = request.clone();
                wrong_run.host = foreign.host.clone();
                wrong_run.admission = foreign.admission.clone();
                foreign_source = Some(
                    crate::validator_inventory_review::InventoryReviewSource::prepare(wrong_run)
                        .await
                        .unwrap(),
                );
                foreign_admission = Some(foreign);
            }
            Some(
                crate::validator_inventory_review::InventoryReviewSource::prepare(request)
                    .await
                    .unwrap(),
            )
        };
        let invocation = Arc::new(StructureInvocation {
            pure,
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
                qualified
                    .as_ref()
                    .map_or_else(|| catalogue.worker.clone(), |source| source.worker_path()),
                next_executor.clone(),
                &root,
                match &qualified {
                    Some(source) if case == 0 => Arc::new(ReceiptRejectionProbe {
                        inner: source.clone(),
                        pure,
                        foreign: foreign_source.clone().unwrap(),
                    })
                        as Arc<dyn InternalRecipeSource>,
                    Some(source) => source.clone() as Arc<dyn InternalRecipeSource>,
                    None => invocation.clone(),
                },
            )
            .unwrap(),
        )
        .await;
        let qualified_observation = match &qualified {
            Some(source) => source.take_observation().await,
            None => None,
        };
        if qualified_observation.is_none() && !invocation.receipt_seen.load(Ordering::SeqCst) {
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
            if case == 0 {
                let quiescence = observed.model_free_quiescence().unwrap();
                crate::bootstrap_quiescence_wire::assert_rejects_corrupt_readback(
                    &quiescence,
                    &program,
                    &serde_json::to_value(observed.receipt.accounting.as_ref().unwrap().task)
                        .unwrap(),
                    observed.report["internal_receipt"].as_str().unwrap(),
                );
            }
            let report = qualified_observation.as_ref().unwrap().report().clone();
            assert_eq!(report["q1_qualification"], false);
            assert_eq!(report["semantic_approval"], false);
            assert_eq!(
                report["aggregation"]["pass"],
                case == 0 || case == 15 || (pure && matches!(case, 22 | 23 | 25 | 29)),
                "case {case}: {report}"
            );
            let rules = report["rules"].as_array().unwrap();
            assert_eq!(rules.len(), if pure { 4 } else { 3 });
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
                case == 0 || case == 6 || case == 7 || case >= 10
            );
            assert_eq!(
                rules
                    .iter()
                    .find(|r| r["rule_id"] == "python_graph")
                    .unwrap()["ok"],
                case == 0 || case == 15 || (pure && case >= 17),
                "case {case}: {report}"
            );
        }
        if pure && case != 9 {
            let report = qualified_observation.as_ref().unwrap().report().clone();
            assert_eq!(
                report["rules"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["rule_id"] == "python_pure_calls")
                    .unwrap()["ok"],
                matches!(case, 0 | 15 | 22 | 23 | 25 | 29),
                "case {case}: {report}"
            );
        }
        if let Some(foreign) = foreign_admission {
            let source = foreign_source.unwrap();
            // Bind first, then change the real live policy before dispatch.
            rig.pool.get().await.unwrap().execute("UPDATE brassclaw_instance_tool_settings SET enabled=false,revision=revision+1 WHERE tool_id=$1", &[&tool]).await.unwrap();
            let denied = bootstrap_recipe_flow::execute(
                &foreign,
                bootstrap_recipe_flow::Request::contained(
                    source.worker_path(),
                    next_executor.clone(),
                    &root,
                    source.clone(),
                )
                .unwrap(),
            )
            .await;
            rig.pool.get().await.unwrap().execute("UPDATE brassclaw_instance_tool_settings SET enabled=true,revision=revision+1 WHERE tool_id=$1", &[&tool]).await.unwrap();
            bootstrap_validation::settle_closed(&foreign).await.unwrap();
            assert!(source.take_observation().await.is_none());
            let denied = denied.unwrap();
            assert!(matches!(denied.receipt.outcome, TaskOutcome::Failed { .. }));
            denied.ports.observe_internal_execution(|execution| {
                assert!(!execution.is_complete());
                assert_eq!(execution.failed_step_id(),Some("0:3"));
                assert!(matches!(execution.host_answers(),[(_,PortAnswer::TerminalError { reason_kind })] if reason_kind == "retained_tool_authorization"));
            }).await.unwrap();
            let rows = rig.pool.get().await.unwrap().query("SELECT attempt_count,phase FROM brassclaw_monty_tool_invocations WHERE run_id=$1", &[&foreign.attempt.run_id.as_uuid()]).await.unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].get::<_, i16>(0), 1);
            assert_eq!(rows[0].get::<_, String>(1), "answered");
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
