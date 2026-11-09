//! Genuine /1 partial-presence preparation retained on V209 before upgrade.
//! This fixture executes only the native W inspector, never the subject.
use super::*;
use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
        retained_validator::RetainedValidatorCoverage,
    },
};
use brassclaw_skills::validator_components::{
    aggregate_draft, collect_draft, structural_presence_draft,
};

pub(super) async fn retain_before_upgrade(pool: &Arc<brassclaw_pg::PgPool>) -> (Uuid, String) {
    let (root, rule, collection, aggregation) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let helpers = [
        structural_presence_draft(rule).unwrap(),
        collect_draft(collection, &["structural_presence".into()]).unwrap(),
        aggregate_draft(aggregation).unwrap(),
    ];
    let store = PgComponentRevisionStore::new(pool.clone());
    let mut references = Vec::new();
    for draft in &helpers {
        references.push(store.retain_packaged_draft(draft).await.unwrap());
    }
    let layout = json!({"format":"recipe-input-layout/1","task_inputs":{"subject_json":{"type":"string","required":true,"checks":[]}},
        "steps":{"0:1":{"name":{"kind":"constant","value":"Legacy partial review"},"description":{"kind":"constant","value":"Preparation only"},"content":{"kind":"task_input","reference":"{{vars.subject_json}}"}},
            "0:2":{"structural_presence":{"kind":"result","step_id":"0:1","path":[]}},
            "0:3":{"required_rule_ids":{"kind":"constant","value":["structural_presence"]},"checks":{"kind":"result","step_id":"0:2","path":[]}}}});
    let recipe = json!({"format":"component-revision/1","uuid":root,"class_code":21,"document":{
        "name":"Legacy preparation only","description":"No execution or class qualification","validates_class_code":22,"consumer_tags":["05:validator"],
        "validator_coverage":{"format":"validator-coverage/1","rules":[{"rule_id":"structural_presence","step_id":"0:1"}],"aggregate_step":"0:3"},
        "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":[],"variable_patterns":[]}],
        "step_descriptions":[{"desc_idx":0,"label":"Partial review","yaml_source":"","steps":[
            {"stepnumber":1,"knowledge":"orchestrator","goal":"Presence only","content":"","type":"component","include":[rule]},
            {"stepnumber":2,"knowledge":"orchestrator","goal":"Collect","content":"","type":"component","include":[collection]},
            {"stepnumber":3,"knowledge":"orchestrator","goal":"Aggregate","content":"","type":"component","include":[aggregation]}]}],
        "input_layouts":{"selected":layout},"invocations":{"selected":{"0:1":"check_structural_presence","0:2":"collect_validator_checks","0:3":"aggregate_validator_checks"}}},
        "dependencies":[rule,collection,aggregation],"association":null});
    references.push(
        store
            .stage(
                &ComponentRevisionDraft::from_json(&recipe.to_string()).unwrap(),
                0,
            )
            .await
            .unwrap(),
    );
    let snapshot = Arc::new(store.read_exact(&[root], &references).await.unwrap());
    let instruction =
        compile_retained_recipe(snapshot, root, "selected", WorkflowClass::RequiresModel).unwrap();
    let worker = brassclaw_monty_host::process::installed_worker().unwrap();
    let inspected = InspectedRetainedProgram::inspect(
        RetainedProgram::Tools(Arc::new(
            prepare_retained_tool_program(instruction).unwrap(),
        )),
        &worker,
    )
    .await
    .unwrap();
    let id = Uuid::new_v4();
    PgReviewSubmissionStore::new(pool.clone())
        .submit(
            &ReviewSubmissionDraft::new(
                id,
                "i01-legacy",
                ComponentRevisionDraft::from_json(&candidate(Uuid::new_v4(), false)).unwrap(),
                None,
                vec![],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let subject =
        PgReviewSubmissionStore::read_validator_subject_in_transaction(&tx, id, REVISION_LIMITS)
            .await
            .unwrap()
            .unwrap();
    let bytes =
        RetainedValidatorCoverage::selection_bytes(&subject, &inspected, REVISION_LIMITS).unwrap();
    selection::retain_in_transaction(&tx, &bytes, REVISION_LIMITS)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    (id, bytes)
}
