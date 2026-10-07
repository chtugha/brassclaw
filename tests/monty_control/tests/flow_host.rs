//! Execute the candidate's actual generic flow code as pure control logic.
//! No component selection, provider response or successful Tool is fabricated.
use std::{collections::BTreeMap, time::Duration};

use brassclaw_monty_host::{
    VmFailure,
    process::ProcessFailure,
    utility::{UtilityOutput, UtilityRequest, execute},
};
use serde_json::{Value, json};

mod support;
use support::{SOURCE, boot, limits, task, worker};

fn reference(step: &str) -> Value {
    json!({"kind":"result","step_id":step,"path":[]})
}
fn constant(value: Value) -> Value {
    json!({"kind":"constant","value":value})
}
fn step(node: &str, id: &str, inputs: Value) -> Value {
    json!({"kind":"step","node_id":node,"step_id":id,"inputs":inputs})
}
fn returning(node: &str, source: Value) -> Value {
    json!({"kind":"return","node_id":node,"source":source})
}
async fn run(
    script: &str,
    values: BTreeMap<String, Value>,
) -> Result<Value, brassclaw_monty_host::utility::UtilityError> {
    // Compile the exact root helpers, excluding only instance entry. Utility
    // workers install no host ports, so accidental dispatch is a real failure.
    let definitions = SOURCE
        .strip_suffix("asyncio.run(_global_main())\n")
        .unwrap();
    let mut bounds = boot(SOURCE).bounds.values;
    // Structured control metadata has more JSON wrapper depth than user data.
    // Keep it below the transport ceiling; flow nesting has its own bound.
    bounds.max_value_depth = 48;
    let output = execute(
        worker(),
        UtilityRequest::Evaluate {
            source: format!("{definitions}\n{script}"),
            inputs: values,
            bounds,
            max_compute_time: Duration::from_secs(2),
        },
        limits(),
    )
    .await?;
    let UtilityOutput::Evaluated { value, .. } = output else {
        panic!("control result")
    };
    Ok(value)
}

#[tokio::test]
async fn full_flow_preflight_rejects_bad_edges_unbounded_expansion_and_unknown_fields() {
    let valid = json!({"format":"recipe-flow/1","body":[
        step("prepare", "0:1", json!({"text":{"kind":"input","name":"user_input"}})),
        returning("complete", reference("0:1"))
    ]});
    let mut forward = valid.clone();
    forward["body"][0]["inputs"]["text"] = reference("0:1");
    let mut unknown = valid.clone();
    unknown["body"][0]["code"] = json!("host.forbidden_effect()");
    let mut missing = valid.clone();
    missing["body"][0]["step_id"] = json!("0:999");
    let mut unreachable = valid.clone();
    unreachable["body"]
        .as_array_mut()
        .unwrap()
        .push(step("late", "0:2", json!({})));
    let mut duplicate = valid.clone();
    duplicate["body"]
        .as_array_mut()
        .unwrap()
        .insert(1, step("prepare_twice", "0:1", json!({})));
    let loop_body = vec![
        step("execute", "0:1", json!({})),
        returning("done", reference("0:1")),
    ];
    let mut bad_bound = json!({"format":"recipe-flow/1","body":[{
        "kind":"repeat","node_id":"cycle","max_iterations":true,
        "initial":constant(Value::Null),"body":loop_body,"update":constant(Value::Null)
    }]});
    let bool_bound = bad_bound.clone();
    bad_bound["body"][0]["max_iterations"] = json!(65);
    let too_large = json!({"format":"recipe-flow/1","body":[{
        "kind":"repeat","node_id":"cycle","max_iterations":64,"initial":constant(Value::Null),
        "body":[{"kind":"foreach","node_id":"calls","source":constant(json!([])),"max_items":256,
            "body":[step("execute", "0:1", json!({}))],"result":reference("0:1")}],
        "update":constant(Value::Null)
    }]});
    let values = BTreeMap::from([
        ("valid".into(), valid),
        (
            "invalid".into(),
            json!([
                forward,
                unknown,
                missing,
                unreachable,
                duplicate,
                bool_bound,
                bad_bound,
                too_large
            ]),
        ),
        ("admitted".into(), task()),
    ]);
    let result = run(
        r#"
inputs = {"user_input": admitted["user_input"], "history": admitted["history"]}
_validate_flow(valid, ["0:1"], inputs)
rejected = 0
for flow in invalid:
    try:
        _validate_flow(flow, ["0:1"], inputs)
    except RuntimeError as error:
        if str(error) != "recipe_composition_failed":
            raise
        rejected = rejected + 1
rejected
"#,
        values,
    )
    .await
    .unwrap();
    assert_eq!(result, json!(8));
}

#[tokio::test]
async fn nonreturning_branches_and_item_scopes_cannot_supply_unexecuted_results() {
    let branch = json!({"format":"recipe-flow/1","body":[
        {"kind":"branch","node_id":"choice","source":constant(json!({"left":null})),"cases":{
            "left":[step("left_step","0:1",json!({}))],
            "right":[step("right_step","0:2",json!({}))]
        }},
        returning("done",reference("0:1"))
    ]});
    let scoped = json!({"format":"recipe-flow/1","body":[
        {"kind":"foreach","node_id":"items","source":constant(json!([])),"max_items":2,
            "body":[step("execute","0:1",json!({"item":{"kind":"item","loop_id":"items","path":[]}}))],
            "result":reference("0:1")},
        returning("done",reference("0:1"))
    ]});
    let result = run(
        r#"
rejected = 0
for case in invalid:
    try:
        _validate_flow(case["flow"], case["step_ids"], {})
    except RuntimeError:
        rejected = rejected + 1
rejected
"#,
        BTreeMap::from([(
            "invalid".into(),
            json!([
                {"flow":branch,"step_ids":["0:1","0:2"]},
                {"flow":scoped,"step_ids":["0:1"]}
            ]),
        )]),
    )
    .await
    .unwrap();
    assert_eq!(result, json!(2));
}

#[tokio::test]
async fn repeat_carries_only_explicit_data_and_foreach_collects_empty_results() {
    // Pure generic controls perform a real two-iteration branch and collect
    // explicit item values. Empty nested groups do not dispatch any component.
    let groups = json!({"kind":"foreach","node_id":"groups","source":constant(json!([])),"max_items":1,
        "body":[returning("unused_return",constant(Value::Null))],"result":constant(Value::Null)});
    let collection = json!({"format":"recipe-flow/1","body":[
        {"kind":"foreach","node_id":"collect","source":constant(json!(["one",null,"üä"])),"max_items":3,
            "body":[groups.clone()],"result":{"kind":"item","loop_id":"collect","path":[]}},
        returning("done",reference("collect"))
    ]});
    let collected = run(
        r#"
_validate_flow(flow, [], {})
state = {"inputs": {}, "results": {}, "previous_result": None}
asyncio.run(_run_flow_nodes("private_task", "pinned_program", flow["body"], state, {}, []))
"#,
        BTreeMap::from([("flow".into(), collection)]),
    )
    .await
    .unwrap();
    assert_eq!(
        collected,
        json!({"returned":true,"value":["one",null,"üä"]})
    );
    let flow = json!({"format":"recipe-flow/1","body":[{
        "kind":"repeat","node_id":"cycle","max_iterations":2,"initial":constant(json!({"continue":{"stop":["two",null,"ß"]}})),
        "body":[{"kind":"branch","node_id":"choice","source":{"kind":"item","loop_id":"cycle","path":[]},
            "cases":{
                "continue":[groups],
                "stop":[returning("done",json!({"kind":"item","loop_id":"cycle","path":["stop"]}))]
            }}],
        "update":{"kind":"item","loop_id":"cycle","path":["continue"]}
    }]});
    let result = run(
        r#"
_validate_flow(flow, [], {})
state = {"inputs": {}, "results": {}, "previous_result": None}
asyncio.run(_run_flow_nodes("private_task", "pinned_program", flow["body"], state, {}, []))
"#,
        BTreeMap::from([("flow".into(), flow)]),
    )
    .await
    .unwrap();
    assert_eq!(result, json!({"returned":true,"value":["two",null,"ß"]}));
}

#[tokio::test]
async fn runtime_invalid_union_missing_field_and_oversized_items_fail_without_truncation() {
    let cases = json!({"left":[returning("done",constant(Value::Null))]});
    let malformed_union = vec![
        json!({"kind":"branch","node_id":"choice","source":constant(json!({"unknown":null})),"cases":cases}),
    ];
    let oversized_items = vec![
        json!({"kind":"foreach","node_id":"items","source":constant(json!([1,2])),"max_items":1,
        "body":[returning("done",constant(Value::Null))],"result":constant(Value::Null)}),
    ];
    let missing_field = vec![returning("done", json!({"kind":"input","name":"missing"}))];
    for nodes in [malformed_union, oversized_items] {
        let error = run("asyncio.run(_run_flow_nodes('task', 'program', nodes, {'inputs': {}, 'results': {}, 'previous_result': None}, {}, []))",
            BTreeMap::from([("nodes".into(),json!(nodes))])).await.unwrap_err();
        assert_eq!(error.kind, ProcessFailure::Vm(VmFailure::Python));
        assert!(
            error
                .diagnostic
                .unwrap()
                .contains("recipe_execution_failed")
        );
    }
    let error = run("asyncio.run(_run_flow_nodes('task', 'program', nodes, {'inputs': {}, 'results': {}, 'previous_result': None}, {}, []))",
        BTreeMap::from([("nodes".into(),json!(missing_field))])).await.unwrap_err();
    assert_eq!(error.kind, ProcessFailure::Vm(VmFailure::Python));
}
