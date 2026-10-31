//! Real retained function subjects and actual Monty checking code. These are
//! structural/declaration observations, not complete joint Q1 or activation.
use std::{collections::BTreeMap, sync::Arc, time::Duration};

use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::{
        retained_inputs::prepare_retained_unbound_program,
        retained_instruction::{WorkflowClass, compile_retained_recipe},
    },
};
use brassclaw_monty_host::{
    process::installed_worker,
    utility::{self, UtilityLimits, UtilityOutput, UtilityRequest},
};
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS, RetainedComponentSnapshot},
    global_bootstrap_components::{UsageComponentIds, packaged_component_id, public_reply_drafts},
    review_submission_store::{
        FunctionReviewPartnerDraft, PgReviewSubmissionStore, ReviewSubmissionDraft,
    },
    revision_store::PgComponentRevisionStore,
    validator_components::{
        preload_definition_draft, recursive_contract_draft, review_subject_reader_draft,
        skill_interface_draft, subject_sources_draft,
    },
    value_contract::ValueContract,
};
use serde_json::{Value, json};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

async fn function_subject(
    pool: &Arc<PgPool>,
    code_primary: bool,
    context: bool,
    extended: bool,
) -> Value {
    function_subject_with(pool, code_primary, context, extended, vec![], |_, _| {}).await
}

async fn function_subject_with(
    pool: &Arc<PgPool>,
    code_primary: bool,
    context: bool,
    extended: bool,
    additional: Vec<ComponentRevisionDraft>,
    mutate: impl FnOnce(&mut Value, &mut Value),
) -> Value {
    let ids = UsageComponentIds {
        recipe: Uuid::new_v4(),
        python_code: Uuid::new_v4(),
        skill: Uuid::new_v4(),
        tool_skill: Uuid::new_v4(),
        tool: Uuid::new_v4(),
        formatter: None,
    };
    let drafts = public_reply_drafts(ids).unwrap();
    let mut code = drafts
        .iter()
        .find(|d| d.uuid() == ids.python_code)
        .unwrap()
        .clone();
    if extended {
        let mut revision: Value = serde_json::from_str(code.exact_bytes()).unwrap();
        revision["document"]["preload"]["exports"]["alternate_publish"] =
            json!({"symbol":"alternate_publish","parameters":["inputs"],"mapping":true});
        let source = revision["document"]["content"].as_str().unwrap().to_owned();
        revision["document"]["content"] = json!(format!(
            "{source}\ndef alternate_publish(inputs):\n    return host.post_reply(answer=inputs['answer'])\n"
        ));
        revision["document"]["input_contract"]["optional_annotation"] =
            json!({"type":"string","required":false,"default":"unchanged","checks":[]});
        code = ComponentRevisionDraft::from_json(&revision.to_string()).unwrap();
    }
    let skill = drafts
        .iter()
        .find(|d| d.uuid() == ids.skill)
        .unwrap()
        .clone();
    let mut skill_value: Value = serde_json::from_str(skill.exact_bytes()).unwrap();
    let mut code_value: Value = serde_json::from_str(code.exact_bytes()).unwrap();
    mutate(&mut skill_value, &mut code_value);
    let skill = ComponentRevisionDraft::from_json(&skill_value.to_string()).unwrap();
    let code = ComponentRevisionDraft::from_json(&code_value.to_string()).unwrap();
    let binding = drafts.iter().find(|d| d.uuid() == ids.tool_skill).unwrap();
    let tool=ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":ids.tool,"class_code":0,
        "document":{"capability_id":"host.post_reply","callable":"host.post_reply"},"dependencies":[],"association":null}).to_string()).unwrap();
    let store = PgComponentRevisionStore::new(pool.clone());
    let tool_ref = store.stage(&tool, 0).await.unwrap();
    let binding_ref = store.stage(binding, 0).await.unwrap();
    let mut dependencies = vec![tool_ref, binding_ref];
    for helper in additional {
        dependencies.push(store.stage(&helper, 0).await.unwrap());
    }
    let (candidate, partner) = if code_primary {
        (code, skill)
    } else {
        (skill, code)
    };
    let partner = if context {
        FunctionReviewPartnerDraft::Context {
            reference: store.stage(&partner, 0).await.unwrap(),
        }
    } else {
        FunctionReviewPartnerDraft::Changed {
            candidate: Box::new(partner),
            base: None,
            expected_previous_version: 0,
        }
    };
    let id = Uuid::new_v4();
    PgReviewSubmissionStore::new(pool.clone())
        .submit(
            &ReviewSubmissionDraft::new_function(
                id,
                "native-function-declaration-review",
                candidate,
                None,
                dependencies,
                0,
                partner,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let subject =
        PgReviewSubmissionStore::read_validator_subject_in_transaction(&tx, id, REVISION_LIMITS)
            .await
            .unwrap()
            .unwrap();
    assert!(subject.is_function_review());
    let value = subject.value().clone();
    tx.commit().await.unwrap();
    value
}

struct Checker {
    inspected: InspectedRetainedProgram,
    source: String,
    output: ValueContract,
    limits: UtilityLimits,
}
impl Checker {
    async fn new(root: ComponentRevisionDraft, dependencies: Vec<ComponentRevisionDraft>) -> Self {
        let recipe_id = Uuid::new_v4();
        let input_contract = root.document()["input_contract"].clone();
        let bindings: serde_json::Map<String, Value> = input_contract
            .as_object()
            .unwrap()
            .keys()
            .map(|name| {
                (
                    name.clone(),
                    json!({"kind":"task_input", "reference":format!("{{{{vars.{name}}}}}")}),
                )
            })
            .collect();
        let recipe=ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":recipe_id,"class_code":21,
            "document":{"name":"private declaration observation only",
                "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":[],"variable_patterns":[]}],
                "step_descriptions":[{"desc_idx":0,"label":"One checking component, no approval","yaml_source":"","steps":[
                    {"stepnumber":1,"knowledge":"orchestrator","goal":"Observe one declared structural check","content":"","type":"component","include":[root.uuid()]}]}],
                "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":input_contract,
                    "steps":{"0:1":bindings}}}},
            "dependencies":[root.uuid()],"association":null}).to_string()).unwrap();
        let output =
            ValueContract::result_from_value(&root.document()["result_contract"], REVISION_LIMITS)
                .unwrap();
        let mut drafts = vec![recipe, root];
        drafts.extend(dependencies);
        let graph = Arc::new(
            RetainedComponentSnapshot::new(
                &[recipe_id],
                drafts
                    .into_iter()
                    .map(|d| d.at_version(1).unwrap())
                    .collect(),
            )
            .unwrap(),
        );
        let program = prepare_retained_unbound_program(
            compile_retained_recipe(graph, recipe_id, "selected", WorkflowClass::Deterministic)
                .unwrap(),
        )
        .unwrap();
        let limits = UtilityLimits::from_execution(Default::default()).unwrap();
        let inspected = InspectedRetainedProgram::inspect_with_limits(
            RetainedProgram::Unbound(Arc::new(program)),
            &installed_worker().unwrap(),
            limits,
        )
        .await
        .unwrap();
        let root = inspected.program().inputs().components()["0:1"].uuid;
        let pure = inspected.pure_component(root).unwrap();
        assert_eq!(pure.components().len(), inspected.preload_order().len());
        let mut source = String::new();
        for id in inspected.preload_order() {
            source.push_str(
                inspected
                    .program()
                    .inputs()
                    .instruction()
                    .snapshot()
                    .revisions()[id]
                    .draft()
                    .document()["content"]
                    .as_str()
                    .unwrap(),
            );
            source.push('\n');
        }
        source.push_str(inspected.invocation("0:1").unwrap());
        source.push_str("\nresult\n");
        Self {
            inspected,
            source,
            output,
            limits,
        }
    }
    async fn invoke(
        &self,
        subject: &Value,
    ) -> Result<Value, brassclaw_monty_host::utility::UtilityError> {
        self.invoke_inputs(&json!({"subject_json":subject.to_string()}))
            .await
    }
    async fn invoke_inputs(
        &self,
        inputs: &Value,
    ) -> Result<Value, brassclaw_monty_host::utility::UtilityError> {
        let bound = self
            .inspected
            .program()
            .inputs()
            .bind_task_inputs(inputs)
            .unwrap();
        let value = utility::execute(
            &installed_worker().unwrap(),
            UtilityRequest::Evaluate {
                source: self.source.clone(),
                inputs: BTreeMap::from([("inputs".into(), bound)]),
                bounds: self.limits.bounds,
                max_compute_time: Duration::from_secs(5),
            },
            self.limits.process,
        )
        .await?;
        let UtilityOutput::Evaluated { value, stdout } = value else {
            panic!("actual Monty checking output required")
        };
        assert!(stdout.is_empty());
        self.output.validate(&value).unwrap();
        Ok(value)
    }
}
fn rule_drafts() -> (ComponentRevisionDraft, Vec<ComponentRevisionDraft>) {
    let recursive =
        recursive_contract_draft(packaged_component_id("validator:recursive-contract")).unwrap();
    let reader =
        review_subject_reader_draft(packaged_component_id("validator:review-subject-reader"))
            .unwrap();
    let definition =
        preload_definition_draft(packaged_component_id("validator:preload-definition")).unwrap();
    (
        skill_interface_draft(
            packaged_component_id("validator:skill-interface"),
            recursive.uuid(),
        )
        .unwrap(),
        vec![recursive, reader, definition],
    )
}
fn skill_document(subject: &mut Value) -> &mut Value {
    if subject["candidate"]["reference"]["class_code"] == 22 {
        &mut subject["function_partner"]["component"]["revision"]["document"]
    } else {
        &mut subject["candidate"]["revision"]["document"]
    }
}

#[tokio::test]
async fn native_skill_interface_and_shared_reader_use_actual_pinned_function_pairs() {
    let database = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let (root, deps) = rule_drafts();
    let checker = Checker::new(root, deps).await;
    let reader =
        review_subject_reader_draft(packaged_component_id("validator:review-subject-reader"))
            .unwrap();
    let extraction =
        Checker::new(subject_sources_draft(Uuid::new_v4()).unwrap(), vec![reader]).await;
    for (code_primary, context) in [(false, false), (true, false), (false, true), (true, true)] {
        let original = function_subject(&database.pool, code_primary, context, false).await;
        assert_eq!(
            checker.invoke(&original).await.unwrap(),
            json!({"rule_id":"skill_interface","ok":true,"errors":[]})
        );
        let code = if code_primary {
            &original["candidate"]
        } else {
            &original["function_partner"]["component"]
        };
        let expected = json!({code["reference"]["uuid"].as_str().unwrap():code["revision"]["document"]["content"]});
        assert_eq!(extraction.invoke(&original).await.unwrap(), expected);
        // Candidate code is data, including a top-level exception. This
        // declaration check is not the separate load/behavior rule.
        let mut unexecuted = original.clone();
        let code = if code_primary {
            &mut unexecuted["candidate"]
        } else {
            &mut unexecuted["function_partner"]["component"]
        };
        code["revision"]["document"]["content"] =
            json!("raise RuntimeError('candidate source must stay data')\n");
        assert_eq!(checker.invoke(&unexecuted).await.unwrap()["ok"], true);
        // Re-entering the same export has no mutable shared fixture state.
        assert_eq!(checker.invoke(&original).await.unwrap()["ok"], true);
    }
    let extended = function_subject(&database.pool, false, false, true).await;
    assert_eq!(checker.invoke(&extended).await.unwrap()["ok"], true);
    let client = database.pool.get().await.unwrap();
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM brassclaw_monty_active_catalogue", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

#[tokio::test]
async fn native_skill_interface_rejects_disagreement_and_shared_reader_rejects_forged_context() {
    let database = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let original = function_subject(&database.pool, false, false, false).await;
    let (root, deps) = rule_drafts();
    let checker = Checker::new(root, deps).await;
    let mut cases = Vec::new();
    for (path, value, diagnostic) in [
        ("body", json!(" \n\t"), "skill.body"),
        ("interface", Value::Null, "complete closed declaration"),
    ] {
        let mut value_subject = original.clone();
        skill_document(&mut value_subject)[path] = value;
        cases.push((value_subject, diagnostic));
    }
    for (field, value, diagnostic) in [
        (
            "format",
            json!("skill-interface/999"),
            "format or implementing identity",
        ),
        (
            "python_code_uuid",
            json!(Uuid::new_v4()),
            "implementing identity",
        ),
        ("exports", json!({}), "nonempty public interface"),
        (
            "dependencies",
            json!(["foreign-helper"]),
            "undeclared or duplicate",
        ),
        (
            "private_symbols",
            json!(["foreign-symbol"]),
            "undeclared or duplicate",
        ),
        ("inputs", json!({}), "differs from the exact usage"),
        (
            "result",
            json!({"type":"integer"}),
            "differs from the exact usage",
        ),
    ] {
        let mut value_subject = original.clone();
        skill_document(&mut value_subject)["interface"][field] = value;
        cases.push((value_subject, diagnostic));
    }
    let mut invalid = original.clone();
    skill_document(&mut invalid)["interface"]["failure"]["max_attempts"] = json!(true);
    cases.push((invalid, "skill.interface.failure"));
    let mut invalid = original.clone();
    skill_document(&mut invalid)["interface"]["exports"]["publish_turn_reply"]["mapping"] =
        json!(1);
    cases.push((invalid, "exact declared signature"));
    let mut invalid = original.clone();
    skill_document(&mut invalid)["interface"]["exports"]["publish_turn_reply"]["mapping"] =
        json!(1);
    invalid["function_partner"]["component"]["revision"]["document"]["preload"]["exports"]["publish_turn_reply"]
        ["mapping"] = json!(1);
    cases.push((invalid, "exact declared signature"));
    let mut invalid = original.clone();
    skill_document(&mut invalid)["interface"]["approved"] = json!(true);
    cases.push((invalid, "complete closed declaration"));
    for (input, diagnostic) in cases {
        let report = checker.invoke(&input).await.unwrap();
        assert_eq!(report["ok"], false);
        assert!(
            report["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e.as_str().unwrap().contains(diagnostic)),
            "{report}"
        );
    }
    let mut forged = original.clone();
    forged["function_partner"]["role"] = json!("system_seed");
    assert!(checker.invoke(&forged).await.is_err());
    let mut forged = original.clone();
    forged["dependencies"]
        .as_array_mut()
        .unwrap()
        .push(original["function_partner"]["component"].clone());
    assert!(checker.invoke(&forged).await.is_err());
    let mut forged = original.clone();
    forged["function_partner"]["component"]["reference"]["version"] = json!(true);
    assert!(checker.invoke(&forged).await.is_err());
    let mut forged = original.clone();
    forged["function_partner"]["role"] = json!("context");
    forged["function_partner"]["base_comparison"] = original["candidate"].clone();
    assert!(checker.invoke(&forged).await.is_err());
    assert_eq!(checker.invoke(&original).await.unwrap()["ok"], true);
}

#[tokio::test]
async fn packaged_function_check_drafts_retain_private_dependency_and_preserve_operator_successors()
{
    let database = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(database.pool.clone());
    let pure_id = packaged_component_id("validator:python-pure-calls");
    let current = brassclaw_skills::validator_components::python_pure_calls_draft(
        pure_id,
        crate::builtin_bootstrap::VALIDATOR_PYTHON_PURE_CALLS_SEED,
    )
    .unwrap();
    // Exact shipped historical source, not a reimplementation of its algorithm.
    // Removing only the newly added declaration restores its original interface.
    let mut previous: Value = serde_json::from_str(current.exact_bytes()).unwrap();
    previous["document"]["content"] = json!(include_str!(
        "../tests/fixtures/validator_python_pure_calls_v1.py"
    ));
    assert!(
        previous["document"]["preload"]["private_functions"]
            .as_object_mut()
            .unwrap()
            .remove("_validator_python_reachable_functions")
            .is_some()
    );
    let previous = ComponentRevisionDraft::from_json(&previous.to_string()).unwrap();
    let historical = store.retain_packaged_draft(&previous).await.unwrap();
    crate::builtin_bootstrap::retain_validator_check_drafts(&database.pool)
        .await
        .unwrap();
    let successor = store.read_revision(pure_id, 2).await.unwrap();
    assert_eq!(historical.version, 1);
    assert_eq!(successor.draft().exact_bytes(), current.exact_bytes());
    assert_eq!(
        store.read_revision(pure_id, 1).await.unwrap().reference(),
        historical
    );
    let mut edited: Value = serde_json::from_str(current.exact_bytes()).unwrap();
    edited["document"]["description"] = json!("operator pure checker override");
    let edited = ComponentRevisionDraft::from_json(&edited.to_string()).unwrap();
    let override_ref = store.stage(&edited, 2).await.unwrap();
    crate::builtin_bootstrap::retain_validator_check_drafts(&database.pool)
        .await
        .unwrap();
    assert_eq!(
        store.read_allocation_head(pure_id).await.unwrap(),
        Some((22, 3))
    );
    for (reference, bytes) in [
        (historical, previous.exact_bytes()),
        (successor.reference(), current.exact_bytes()),
        (override_ref, edited.exact_bytes()),
    ] {
        assert_eq!(
            store
                .read_revision(reference.uuid, reference.version)
                .await
                .unwrap()
                .draft()
                .exact_bytes(),
            bytes
        );
    }
    let (rule, deps) = rule_drafts();
    for draft in std::iter::once(&rule).chain(deps.iter()) {
        let original = store.read_revision(draft.uuid(), 1).await.unwrap();
        assert_eq!(original.draft().exact_bytes(), draft.exact_bytes());
        let mut edited: Value = serde_json::from_str(draft.exact_bytes()).unwrap();
        edited["document"]["description"] = json!("operator override");
        let edited = ComponentRevisionDraft::from_json(&edited.to_string()).unwrap();
        let second = store.stage(&edited, 1).await.unwrap();
        crate::builtin_bootstrap::retain_validator_check_drafts(&database.pool)
            .await
            .unwrap();
        assert_eq!(
            store.read_allocation_head(draft.uuid()).await.unwrap(),
            Some((22, 2))
        );
        assert_eq!(
            store
                .read_revision(draft.uuid(), 1)
                .await
                .unwrap()
                .reference(),
            original.reference()
        );
        assert_eq!(
            store
                .read_revision(draft.uuid(), 2)
                .await
                .unwrap()
                .reference(),
            second
        );
    }
    let client = database.pool.get().await.unwrap();
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

/// Reuses the actual retained checking harness and actual preparation owner.
/// /2 admission/sealed-Q1 is deliberately NOT supplied by this fixture.
struct CallTargetChecks {
    extractor: Checker,
    definitions: Checker,
    contracts: Checker,
    graph: Checker,
    interface: Checker,
    targets: Checker,
    native: crate::source_inspection::ContainedSourceInspection,
}
impl CallTargetChecks {
    async fn new() -> Self {
        use brassclaw_skills::validator_components::{
            python_contracts_draft, python_definitions_draft, python_graph_draft,
            python_pure_calls_draft, skill_call_targets_draft,
        };
        let recursive =
            recursive_contract_draft(packaged_component_id("validator:recursive-contract"))
                .unwrap();
        let reader =
            review_subject_reader_draft(packaged_component_id("validator:review-subject-reader"))
                .unwrap();
        let definition =
            preload_definition_draft(packaged_component_id("validator:preload-definition"))
                .unwrap();
        let pure_calls = python_pure_calls_draft(
            packaged_component_id("validator:python-pure-calls"),
            crate::builtin_bootstrap::VALIDATOR_PYTHON_PURE_CALLS_SEED,
        )
        .unwrap();
        let extractor = Checker::new(
            subject_sources_draft(Uuid::new_v4()).unwrap(),
            vec![reader.clone()],
        )
        .await;
        let definitions = Checker::new(
            python_definitions_draft(
                Uuid::new_v4(),
                definition.uuid(),
                crate::builtin_bootstrap::VALIDATOR_PYTHON_DEFINITIONS_SEED,
            )
            .unwrap(),
            vec![reader.clone(), definition.clone()],
        )
        .await;
        let contracts = Checker::new(
            python_contracts_draft(
                Uuid::new_v4(),
                recursive.uuid(),
                crate::builtin_bootstrap::VALIDATOR_PYTHON_CONTRACTS_SEED,
            )
            .unwrap(),
            vec![reader.clone(), recursive.clone()],
        )
        .await;
        let graph = Checker::new(
            python_graph_draft(
                Uuid::new_v4(),
                crate::builtin_bootstrap::VALIDATOR_PYTHON_GRAPH_SEED,
            )
            .unwrap(),
            vec![reader.clone()],
        )
        .await;
        let interface = Checker::new(
            skill_interface_draft(Uuid::new_v4(), recursive.uuid()).unwrap(),
            vec![reader.clone(), recursive, definition],
        )
        .await;
        let targets = Checker::new(
            skill_call_targets_draft(Uuid::new_v4()).unwrap(),
            vec![reader, pure_calls],
        )
        .await;
        let settings =
            Arc::new(crate::host_primitive_settings::AcknowledgedPrimitiveSettings::new());
        settings
            .prepare_bootstrap(brassclaw_host_runtime::HostPrimitiveSettings {
                revision: 1,
                limits: Default::default(),
            })
            .unwrap();
        let native = crate::source_inspection::ContainedSourceInspection::new();
        native.bind(settings).await.unwrap();
        Self {
            extractor,
            definitions,
            contracts,
            graph,
            interface,
            targets,
            native,
        }
    }
    async fn run(&self, subject: &Value) -> (Value, Value) {
        use brassclaw_host_runtime::SourceInspectionBackend;
        use sha2::Digest as _;
        let sources = self.extractor.invoke(subject).await.unwrap();
        let inspection = self
            .native
            .inspect_sources(sources.as_object().unwrap())
            .await
            .unwrap();
        assert_eq!(inspection.usage.process_count, 1);
        let inventory = inspection.output.as_str().unwrap();
        let report: Value = serde_json::from_str(inventory).unwrap();
        let prepared = self.native.prepare().await.unwrap();
        assert_eq!(report["preparation_revision"], prepared.revision);
        assert_eq!(
            report["worker_checksum"],
            hex::encode(prepared.worker.checksum())
        );
        assert_eq!(
            report["sources"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            sources.as_object().unwrap().keys().collect::<Vec<_>>()
        );
        for (id, source) in sources.as_object().unwrap() {
            let item = &report["sources"][id.as_str()];
            let checksum = if item["kind"] == "inspected" {
                &item["structure"]["source_checksum"]
            } else {
                &item["source_checksum"]
            };
            assert_eq!(
                *checksum,
                json!(hex::encode(sha2::Sha256::digest(
                    source.as_str().unwrap().as_bytes()
                )))
            );
        }
        let base = json!({"subject_json":subject.to_string(),"inventory_json":inventory});
        let definitions = self.definitions.invoke_inputs(&base).await.unwrap();
        let contracts = self
            .contracts
            .invoke_inputs(&json!({"subject_json":subject.to_string()}))
            .await
            .unwrap();
        let profile =
            serde_json::to_value(brassclaw_engine::executor::retained_profile::profile()).unwrap();
        let graph = self
            .graph
            .invoke_inputs(&json!({
                "subject_json":subject.to_string(),"inventory_json":inventory,"profile":profile,
                "definitions":definitions,"contracts":contracts,
            }))
            .await
            .unwrap();
        let interface = self.interface.invoke(subject).await.unwrap();
        let inputs = json!({"subject_json":subject.to_string(),"inventory_json":inventory,
            "profile":profile,"graph":graph,"interface":interface});
        let result = self.targets.invoke_inputs(&inputs).await.unwrap();
        (result, inputs)
    }
}
fn set_usage_source(code: &mut Value, source: &str) {
    code["document"]["content"] = json!(source);
}
fn add_private_relay(skill: &mut Value, code: &mut Value, target: &str) {
    set_usage_source(
        code,
        &format!(
            "def publish_turn_reply(inputs):\n    return relay(inputs['answer'])\n\ndef relay(answer):\n    return host.{target}(answer=answer)\n"
        ),
    );
    code["document"]["preload"]["private_functions"]["relay"] = json!(["answer"]);
    skill["document"]["interface"]["private_symbols"] = json!(["relay"]);
}
fn dependency_relay(id: Uuid, target: &str) -> ComponentRevisionDraft {
    ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":id,"class_code":22,
        "document":{"name":"review context helper","content":format!("def dependency_relay(answer):\n    return host.{target}(answer=answer)\n"),
            "preload":{"format":"python-preload/2","exports":{},"private_functions":{"dependency_relay":["answer"]},
                "constants":[],"imports":[],"dependencies":[],"default_export":null}},
        "dependencies":[],"association":null,
    }).to_string()).unwrap()
}
fn use_dependency_relay(skill: &mut Value, code: &mut Value, id: Uuid) {
    set_usage_source(
        code,
        "def publish_turn_reply(inputs):\n    return dependency_relay(inputs['answer'])\n",
    );
    code["dependencies"] = json!([id]);
    code["document"]["preload"]["dependencies"] = json!([id]);
    skill["document"]["interface"]["dependencies"] = json!([id]);
}

#[tokio::test]
async fn native_skill_call_targets_compare_actual_joint_sources_and_reachable_helpers() {
    let database = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let checks = CallTargetChecks::new().await;
    for (code_primary, context) in [(false, false), (true, false), (false, true), (true, true)] {
        let subject = function_subject(&database.pool, code_primary, context, false).await;
        let (result, inputs) = checks.run(&subject).await;
        assert_eq!(inputs["graph"]["ok"], true);
        assert_eq!(inputs["interface"]["ok"], true);
        assert_eq!(
            result,
            json!({"rule_id":"skill_call_targets","ok":true,"errors":[]})
        );
    }
    for (target, expected) in [("post_reply", true), ("different_tool", false)] {
        let subject = function_subject_with(
            &database.pool,
            false,
            false,
            false,
            vec![],
            |skill, code| {
                add_private_relay(skill, code, target);
            },
        )
        .await;
        let (result, inputs) = checks.run(&subject).await;
        assert_eq!(inputs["graph"]["ok"], true);
        assert_eq!(inputs["interface"]["ok"], true);
        assert_eq!(
            result["ok"], expected,
            "actual private helper target {target}"
        );
        let id = Uuid::new_v4();
        let subject = function_subject_with(
            &database.pool,
            true,
            true,
            false,
            vec![dependency_relay(id, target)],
            |skill, code| {
                use_dependency_relay(skill, code, id);
            },
        )
        .await;
        let original = subject["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["reference"]["uuid"] == json!(id))
            .unwrap();
        let original_bytes = original["revision_bytes"].clone();
        // An independently staged newer helper does not replace this review's
        // exact context or turn it into another approval candidate.
        let store = PgComponentRevisionStore::new(database.pool.clone());
        store
            .stage(&dependency_relay(id, "unselected_successor"), 1)
            .await
            .unwrap();
        let (result, inputs) = checks.run(&subject).await;
        assert_eq!(inputs["graph"]["ok"], true);
        assert_eq!(inputs["interface"]["ok"], true);
        assert_eq!(result["ok"], expected, "actual dependency target {target}");
        assert_eq!(original["revision_bytes"], original_bytes);
    }
}

#[tokio::test]
async fn native_skill_call_targets_keep_export_selection_and_no_cardinality_claim() {
    let database = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let checks = CallTargetChecks::new().await;
    for (case, expected) in [
        ("unused_private", true),
        ("second_export", false),
        ("same_target_twice", true),
        ("no_host_site", true),
    ] {
        let subject = function_subject_with(&database.pool,false,false,false,vec![],|skill,code| match case {
            "unused_private" => {
                let content = code["document"]["content"].as_str().unwrap().to_owned();
                set_usage_source(code,&format!("# Unicode ü and host.different_tool are data\n{content}\n\ndef unused(answer):\n    return host.different_tool(answer=answer)\n"));
                code["document"]["preload"]["private_functions"]["unused"] = json!(["answer"]);
                skill["document"]["interface"]["private_symbols"] = json!(["unused"]);
            }
            "second_export" => {
                let content = code["document"]["content"].as_str().unwrap().to_owned();
                set_usage_source(code,&format!("{content}\n\ndef second(inputs):\n    return host.different_tool(answer=inputs['answer'])\n"));
                let export = json!({"symbol":"second","parameters":["inputs"],"mapping":true});
                code["document"]["preload"]["exports"]["second"] = export.clone();
                skill["document"]["interface"]["exports"]["second"] = export;
            }
            "same_target_twice" => set_usage_source(code,"def publish_turn_reply(inputs):\n    host.post_reply(answer=inputs['answer'])\n    return host.post_reply(answer=inputs['answer'])\n"),
            "no_host_site" => set_usage_source(code,"def publish_turn_reply(inputs):\n    return inputs['answer']\n"),
            _ => unreachable!(),
        }).await;
        let (result, inputs) = checks.run(&subject).await;
        assert_eq!(inputs["graph"]["ok"], true);
        assert_eq!(inputs["interface"]["ok"], true);
        assert_eq!(
            result["ok"], expected,
            "target-only case {case}, not full Skill/runtime approval"
        );
    }
}

#[tokio::test]
async fn native_skill_call_targets_fail_closed_on_missing_inputs_and_prerequisites() {
    let database = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let checks = CallTargetChecks::new().await;
    let subject = function_subject(&database.pool, false, false, false).await;
    let (result, inputs) = checks.run(&subject).await;
    assert_eq!(result["ok"], true);
    for field in ["graph", "interface", "compile"] {
        let invalid = function_subject_with(&database.pool,false,false,false,vec![],|skill,code| {
            if field == "compile" {
                set_usage_source(code,"def invalid_source(:\n");
            } else if field == "graph" {
                // Native source signature disagrees with the declared preload;
                // the real definitions result makes the graph prerequisite fail.
                set_usage_source(code,"def publish_turn_reply(other):\n    return host.post_reply(answer=other)\n");
            } else {
                skill["document"]["interface"]["exports"] = json!({"missing_export":{
                    "symbol":"missing_export","parameters":["inputs"],"mapping":true,
                }});
            }
        }).await;
        let (failed, actual_inputs) = checks.run(&invalid).await;
        let prerequisite = if field == "compile" { "graph" } else { field };
        assert_eq!(actual_inputs[prerequisite]["ok"], false);
        assert_eq!(failed["ok"], false, "actual failed prerequisite {field}");
    }
    let mut missing = inputs.clone();
    let mut inventory: Value =
        serde_json::from_str(inputs["inventory_json"].as_str().unwrap()).unwrap();
    inventory["sources"] = json!({});
    missing["inventory_json"] = json!(inventory.to_string());
    assert!(checks.targets.invoke_inputs(&missing).await.is_err());
    let cycle = function_subject_with(&database.pool,false,false,false,vec![],|skill,code| {
        add_private_relay(skill,code,"post_reply");
        set_usage_source(code,"def publish_turn_reply(inputs):\n    return relay(inputs['answer'])\n\ndef relay(answer):\n    return relay(answer)\n");
    }).await;
    let (result, _) = checks.run(&cycle).await;
    assert_eq!(result["ok"], false);
    assert!(
        result["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e.as_str().unwrap().contains("recursive"))
    );
    let value_edge = function_subject_with(&database.pool,false,false,false,vec![],|skill,code| {
        add_private_relay(skill,code,"different_tool");
        set_usage_source(code,"def publish_turn_reply(inputs):\n    return relay\n\ndef relay(answer):\n    return host.different_tool(answer=answer)\n");
    }).await;
    let (result, _) = checks.run(&value_edge).await;
    assert_eq!(
        result["ok"], false,
        "function-value edges cannot hide a different Host target"
    );
    let nested = function_subject_with(&database.pool,false,false,false,vec![],|_,code| {
        set_usage_source(code,"def publish_turn_reply(inputs):\n    def hidden():\n        return host.post_reply(answer=inputs['answer'])\n    return hidden()\n");
    }).await;
    let (result, _) = checks.run(&nested).await;
    assert_eq!(result["ok"], false);
    // No claim that a forged caller inventory can be authenticated here. The
    // existing joint-admission/observe_subject refusals remain unchanged.
}
