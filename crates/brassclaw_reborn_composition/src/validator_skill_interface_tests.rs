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
    let binding = drafts.iter().find(|d| d.uuid() == ids.tool_skill).unwrap();
    let tool=ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":ids.tool,"class_code":0,
        "document":{"capability_id":"host.post_reply","callable":"host.post_reply"},"dependencies":[],"association":null}).to_string()).unwrap();
    let store = PgComponentRevisionStore::new(pool.clone());
    let tool_ref = store.stage(&tool, 0).await.unwrap();
    let binding_ref = store.stage(binding, 0).await.unwrap();
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
                vec![tool_ref, binding_ref],
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
        let recipe=ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":recipe_id,"class_code":21,
            "document":{"name":"private declaration observation only",
                "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":[],"variable_patterns":[]}],
                "step_descriptions":[{"desc_idx":0,"label":"One checking component, no approval","yaml_source":"","steps":[
                    {"stepnumber":1,"knowledge":"orchestrator","goal":"Observe one declared structural check","content":"","type":"component","include":[root.uuid()]}]}],
                "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":{"subject_json":{"type":"string","required":true,"checks":[]}},
                    "steps":{"0:1":{"subject_json":{"kind":"task_input","reference":"{{vars.subject_json}}"}}}}}},
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
        let bound = self
            .inspected
            .program()
            .inputs()
            .bind_task_inputs(&json!({"subject_json":subject.to_string()}))
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
    crate::builtin_bootstrap::retain_validator_check_drafts(&database.pool)
        .await
        .unwrap();
    let store = PgComponentRevisionStore::new(database.pool.clone());
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
