//! Fixed installation fixture definitions, never candidate-supplied expectations.
use super::*;
use brassclaw_skills::component_revision::ComponentRevisionDraft;

pub(super) const RECEIPT_SOURCE: &str = include_str!("bootstrap_checker_receipt.py");
pub(super) fn receipt_id() -> Uuid {
    packaged_component_id("validator:structural:qualification:receipt")
}
fn rule(errors: &[&str]) -> Value {
    json!({"rule_id":"structural_presence","ok":errors.is_empty(),"errors":errors})
}
fn rule_schema() -> Value {
    json!({"type":"object","checks":[],"allow_extra_fields":false,"fields":{
        "rule_id":{"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]},
        "ok":{"type":"boolean","required":true,"checks":[]},
        "errors":{"type":"list","required":true,"checks":[],"items":{"type":"string","checks":[{"kind":"min_length","value":1}]}}
    }})
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Case {
    pub slot: String,
    pub input: Value,
    pub expected: Value,
}
fn document(class: i16) -> Value {
    let mut doc =
        json!({"name":"checking fixture Ü","description":"quotes '\" % {{vars.data}}\nnot source"});
    match class {
        0 => {
            doc["capability_id"] = json!("fixture.operation");
            doc["effect_type"] = json!("read");
            doc["param_schema"] = json!({"type":"object"});
        }
        1..=3 => doc["body"] = json!("Purpose: usage of a declared export"),
        13 => doc["content"] = json!("binding metadata"),
        21 => doc["step_descriptions"] = json!([{"steps":[]}]),
        22 => doc["content"] = json!("def sample(inputs):\n    return inputs\n"),
        23 => doc["overview_doc"] = json!("catalogue overview"),
        _ => unreachable!("closed class set"),
    }
    doc
}
fn subject(doc: &Value) -> Value {
    json!({"subject_json":json!({"candidate":{"revision":{"document":doc}}}).to_string()})
}
pub(super) fn cases(class: i16) -> Vec<Case> {
    let valid = document(class);
    let mut values = vec![(valid.clone(), rule(&[]))];
    for name in [json!(true), json!(1), json!({"opaque":"name"})] {
        let mut doc = valid.clone();
        doc["name"] = name;
        values.push((doc, rule(&[]))); // This checker coerces truthy names; it is not candidate typing.
    }
    for (field, error) in [
        ("name", "name is empty"),
        ("description", "description is empty"),
    ] {
        for replacement in [
            None,
            Some(json!("")),
            Some(json!(" \t\n")),
            Some(Value::Null),
            Some(json!(false)),
            Some(json!(0)),
        ] {
            let mut doc = valid.clone();
            match replacement {
                Some(value) => {
                    doc[field] = value;
                }
                None => {
                    doc.as_object_mut().unwrap().remove(field);
                }
            }
            values.push((doc, rule(&[error])));
        }
    }
    let (field, error) = match class {
        0 => ("capability_id", "capability_id is absent or empty"),
        1..=3 => ("body", "body is absent or empty"),
        13 => ("content", "prose content field is absent or empty"),
        21 => ("step_descriptions", "step_descriptions is absent or empty"),
        22 => ("content", "content is empty"),
        23 => ("overview_doc", "overview_doc is absent or empty"),
        _ => unreachable!(),
    };
    let mut replacements = vec![
        None,
        Some(Value::Null),
        Some(json!(false)),
        Some(json!(0)),
        Some(json!("")),
    ];
    if class != 21 {
        replacements.push(Some(json!(" \t\n")));
    } else {
        replacements.extend([
            Some(json!([])),
            Some(json!({"steps":[1]})),
            Some(json!("non-list")),
        ]);
    }
    for replacement in replacements {
        let mut doc = valid.clone();
        match replacement {
            Some(value) => {
                doc[field] = value;
            }
            None => {
                doc.as_object_mut().unwrap().remove(field);
            }
        }
        values.push((doc, rule(&[error])));
    }
    match class {
        0 => {
            for (field, error) in [
                (
                    "effect_type",
                    "effect_type is absent or has unsupported value",
                ),
                ("param_schema", "param_schema is absent"),
            ] {
                let mut doc = valid.clone();
                doc.as_object_mut().unwrap().remove(field);
                values.push((doc, rule(&[error])));
            }
            for effect in [json!("write"), json!("read-write"), json!("none")] {
                let mut doc = valid.clone();
                doc["effect_type"] = effect;
                values.push((doc, rule(&[])));
            }
            for effect in [Value::Null, json!("unknown"), json!(false)] {
                let mut doc = valid.clone();
                doc["effect_type"] = effect;
                values.push((
                    doc,
                    rule(&["effect_type is absent or has unsupported value"]),
                ));
            }
            for schema in [Value::Null, json!({}), json!([]), json!(false)] {
                let mut doc = valid.clone();
                doc["param_schema"] = schema;
                values.push((doc, rule(&["param_schema is absent"])));
            }
        }
        1..=3 => {
            for body in [
                "def literal text",
                "literal export text",
                "literal USAGE text",
            ] {
                let mut doc = valid.clone();
                doc["body"] = json!(body);
                values.push((doc, rule(&[])));
            }
            let mut doc = valid.clone();
            doc["body"] = json!("marker-free prose");
            values.push((
                doc,
                rule(&["body does not appear to declare a preloadable export function"]),
            ));
        }
        22 => {
            let mut doc = valid.clone();
            doc["content"] = json!("marker-free prose");
            values.push((
                doc,
                rule(&["PythonCode content does not declare any function (missing 'def')"]),
            ));
            let mut doc = valid.clone();
            doc["content"] = json!("# def is only a comment, and is intentionally a presence pass");
            values.push((doc, rule(&[])));
        }
        _ => {}
    }
    let mut many = valid.clone();
    many["name"] = json!("");
    many["description"] = json!("");
    values.push((many, rule(&["name is empty", "description is empty"])));
    values.push((valid, rule(&[]))); // Negative results must not leak into a later invocation.
    values
        .into_iter()
        .enumerate()
        .map(|(i, (doc, expected))| Case {
            slot: format!("case_{i:04}"),
            input: subject(&doc),
            expected,
        })
        .collect()
}
pub(super) fn receipt_draft() -> Result<ComponentRevisionDraft, RebornBuildError> {
    let mut inputs = serde_json::Map::new();
    inputs.insert(
        "expected_json".into(),
        json!({"type":"string","required":true,"checks":[]}),
    );
    for slot in case_slots() {
        let mut schema = rule_schema();
        schema["required"] = json!(false);
        schema["nullable"] = json!(true);
        schema["default"] = Value::Null;
        inputs.insert(slot, schema);
    }
    draft(
        receipt_id(),
        22,
        json!({"name":"encode-structural-checker-fixtures",
        "description":"Compare independently specified fixture expectations with actual checking-export results and encode observed step results. Declared unused slots default to null; every matrix-selected slot must contain its actual complete non-null rule result. Null never supplies a passing result; reject non-null foreign slots. This qualifies neither candidate classes nor arbitrary reports; no host calls or approval writes.",
        "content":RECEIPT_SOURCE,"includes":[],"dependency_registry":null,"input_contract":inputs,"result_contract":{"type":"string","checks":[]},
        "preload":{"format":"python-preload/2","exports":{"encode_structural_checker_cases":{"symbol":"encode_structural_checker_cases","parameters":["inputs"],"mapping":true}},"private_functions":{},"constants":[],"imports":["json"],"dependencies":[],"default_export":"encode_structural_checker_cases"}}),
        vec![],
    )
}
fn case_slots() -> BTreeSet<String> {
    brassclaw_skills::validator_components::VALIDATOR_CLASS_CODES
        .iter()
        .flat_map(|c| cases(*c).into_iter().map(|c| c.slot))
        .collect()
}
fn draft(
    id: Uuid,
    class: i32,
    document: Value,
    dependencies: Vec<Uuid>,
) -> Result<ComponentRevisionDraft, RebornBuildError> {
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":class,"document":document,"dependencies":dependencies,"association":null}).to_string()).map_err(invalid)
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Suite {
    pub variant: String,
    pub steps: Vec<FixtureStep>,
    pub expected_receipt: Option<Value>,
    pub python_stop: bool,
    pub expected_python_line: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FixtureStep {
    pub id: String,
    pub component: Uuid,
    pub export: String,
    pub constants: Value,
    pub results: BTreeMap<String, String>,
}
fn step(id: usize, component: Uuid, export: &str, constants: Value) -> FixtureStep {
    FixtureStep {
        id: format!("0:{id}"),
        component,
        export: export.into(),
        constants,
        results: BTreeMap::new(),
    }
}
pub(super) fn checker_suites(class: i16, root: Uuid) -> Vec<Suite> {
    let cases = cases(class);
    let mut steps = Vec::new();
    let mut expected = Vec::new();
    let mut edges = BTreeMap::new();
    for (i, case) in cases.iter().enumerate() {
        let step = step(i + 1, root, "check_structural_presence", case.input.clone());
        expected.push(json!({"slot":case.slot,"step":step.id,"expected":case.expected}));
        edges.insert(case.slot.clone(), step.id.clone());
        steps.push(step);
    }
    let mut encode = step(
        steps.len() + 1,
        receipt_id(),
        "encode_structural_checker_cases",
        json!({"expected_json":Value::Array(expected).to_string()}),
    );
    encode.results = edges;
    steps.push(encode);
    let mut suites = vec![Suite {
        variant: "selected".into(),
        steps,
        expected_receipt: None,
        python_stop: false,
        expected_python_line: None,
    }];
    for (name, source, line) in [
        (
            "stop_json",
            "!",
            "json.JSONDecodeError: Expecting value: line 1 column 1 (char 0)",
        ),
        ("stop_document", "{}", "KeyError: 'candidate'"),
    ] {
        suites.push(Suite {
            variant: name.into(),
            steps: vec![step(
                1,
                root,
                "check_structural_presence",
                json!({"subject_json":source}),
            )],
            expected_receipt: None,
            python_stop: true,
            expected_python_line: Some(line.into()),
        });
    }
    suites
}
pub(super) fn receipt_suites() -> Vec<Suite> {
    let yes = rule(&[]);
    let no = rule(&["actual diagnostic Ü"]);
    let mut suites = Vec::new();
    for (name, values) in [
        ("one", vec![yes.clone()]),
        ("negative", vec![no.clone()]),
        ("multiple", vec![yes.clone(), no.clone()]),
    ] {
        let mut args = json!({});
        let mut matrix = Vec::new();
        let mut output = serde_json::Map::new();
        for (i, value) in values.into_iter().enumerate() {
            let slot = format!("case_{i:04}");
            let id = format!("0:{}", i + 1);
            args[&slot] = value.clone();
            matrix.push(json!({"slot":slot,"step":id,"expected":value}));
            output.insert(id, value);
        }
        args["expected_json"] = json!(Value::Array(matrix).to_string());
        suites.push(Suite {
            variant: name.into(),
            steps: vec![step(
                1,
                receipt_id(),
                "encode_structural_checker_cases",
                args,
            )],
            expected_receipt: Some(json!({"format":"validator-step-results/1","results":output})),
            python_stop: false,
            expected_python_line: None,
        });
    }
    for (name, expected_json, actual) in [
        (
            "stop_mismatch",
            json!([{"slot":"case_0000","step":"0:1","expected":yes}]).to_string(),
            no,
        ),
        (
            "stop_contradictory",
            json!([{"slot":"case_0000","step":"0:1","expected":yes}]).to_string(),
            json!({"rule_id":"structural_presence","ok":true,"errors":["contradiction"]}),
        ),
        ("stop_matrix", "[]".into(), yes),
    ] {
        suites.push(Suite {
            variant: name.into(),
            steps: vec![step(
                1,
                receipt_id(),
                "encode_structural_checker_cases",
                json!({"expected_json":expected_json,"case_0000":actual}),
            )],
            expected_receipt: None,
            python_stop: true,
            expected_python_line: Some(
                match name {
                    "stop_mismatch" => "ValueError: checker_fixture_outcome_differs",
                    "stop_contradictory" => "ValueError: checker_fixture_result_contradictory",
                    "stop_matrix" => "ValueError: checker_fixture_matrix_invalid",
                    _ => unreachable!("closed receipt fixture set"),
                }
                .into(),
            ),
        });
    }
    let yes = rule(&[]);
    let row = json!({"slot":"case_0000","step":"0:1","expected":yes});
    let extra_stops = [
        (
            "stop_duplicate_slot",
            json!([row.clone(), {"slot":"case_0000","step":"0:2","expected":yes}]),
            json!({"case_0000":yes}),
            "ValueError: checker_fixture_identity_invalid",
        ),
        (
            "stop_duplicate_step",
            json!([row.clone(), {"slot":"case_0001","step":"0:1","expected":yes}]),
            json!({"case_0000":yes,"case_0001":yes}),
            "ValueError: checker_fixture_identity_invalid",
        ),
        (
            "stop_missing_slot",
            json!([{"slot":"case_0001","step":"0:1","expected":yes}]),
            json!({"case_0000":yes}),
            "ValueError: checker_fixture_result_invalid",
        ),
        (
            "stop_foreign_slot",
            json!([row.clone()]),
            json!({"case_0000":yes,"case_0001":yes}),
            "ValueError: checker_fixture_coverage_differs",
        ),
        (
            "stop_bad_row",
            json!([{"slot":"case_0000","step":"0:1"}]),
            json!({"case_0000":yes}),
            "ValueError: checker_fixture_matrix_invalid",
        ),
        (
            "stop_bool_number",
            json!([{"slot":"case_0000","step":"0:1","expected":{"rule_id":"structural_presence","ok":1,"errors":[]}}]),
            json!({"case_0000":yes}),
            "ValueError: checker_fixture_result_invalid",
        ),
        (
            "stop_empty_diagnostic",
            json!([{"slot":"case_0000","step":"0:1","expected":{"rule_id":"structural_presence","ok":false,"errors":[""]}}]),
            json!({"case_0000":yes}),
            "ValueError: checker_fixture_diagnostic_invalid",
        ),
    ];
    for (name, matrix, mut inputs, line) in extra_stops {
        inputs["expected_json"] = json!(matrix.to_string());
        suites.push(Suite {
            variant: name.into(),
            steps: vec![step(
                1,
                receipt_id(),
                "encode_structural_checker_cases",
                inputs,
            )],
            expected_receipt: None,
            python_stop: true,
            expected_python_line: Some(line.into()),
        });
    }
    suites
}
pub(super) fn recipe(
    root: Uuid,
    class: Option<i16>,
    suites: &[Suite],
) -> Result<ComponentRevisionDraft, RebornBuildError> {
    let id = packaged_component_id(&class.map_or_else(
        || "validator:structural:qualification:receipt-recipe".into(),
        |c| format!("validator:structural:qualification:class-{c}"),
    ));
    let mut variants = Vec::new();
    let mut descriptions = Vec::new();
    let mut layouts = serde_json::Map::new();
    let mut invocations = serde_json::Map::new();
    let mut dependencies = BTreeSet::from([root]);
    for (desc, suite) in suites.iter().enumerate() {
        variants.push(json!({"variant_key":suite.variant,"step_link":format!("{desc}:1-{desc}:E"),"intent_examples":[],"variable_patterns":[]}));
        let mut steps = Vec::new();
        let mut bindings = serde_json::Map::new();
        let mut calls = serde_json::Map::new();
        for (ordinal, fixture) in suite.steps.iter().enumerate() {
            let actual_id = format!("{desc}:{}", ordinal + 1);
            dependencies.insert(fixture.component);
            steps.push(json!({"stepnumber":ordinal+1,"knowledge":"orchestrator","goal":"Invoke the fixed installation fixture","content":"","type":"component","include":[fixture.component]}));
            let mut inputs = serde_json::Map::new();
            for (name, value) in fixture
                .constants
                .as_object()
                .ok_or_else(|| invalid("fixture constants must be object"))?
            {
                inputs.insert(name.clone(), json!({"kind":"constant","value":value}));
            }
            for (name, producer) in &fixture.results {
                let (_, number) = producer
                    .split_once(':')
                    .ok_or_else(|| invalid("fixture result step malformed"))?;
                inputs.insert(
                    name.clone(),
                    json!({"kind":"result","step_id":format!("{desc}:{number}"),"path":[]}),
                );
            }
            if fixture.component == receipt_id() {
                for slot in case_slots() {
                    inputs
                        .entry(slot)
                        .or_insert_with(|| json!({"kind":"consumer_default"}));
                }
            }
            bindings.insert(actual_id.clone(), Value::Object(inputs));
            calls.insert(actual_id, json!(fixture.export));
        }
        descriptions.push(json!({"desc_idx":desc,"label":"Closed installation export fixtures","yaml_source":"","steps":steps}));
        layouts.insert(
            suite.variant.clone(),
            json!({"format":"recipe-input-layout/1","task_inputs":{},"steps":bindings}),
        );
        invocations.insert(suite.variant.clone(), Value::Object(calls));
    }
    draft(
        id,
        21,
        json!({"name":"structural-checker-own-qualification","description":"Closed installation own-export fixtures only; not candidate-class Q1 or an active route.","variants":variants,"step_descriptions":descriptions,"input_layouts":layouts,"invocations":invocations}),
        dependencies.into_iter().collect(),
    )
}
