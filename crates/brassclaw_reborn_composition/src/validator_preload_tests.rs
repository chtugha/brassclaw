//! Native parser facts and unchanged pure checking code, without candidate
//! invocation, fabricated review evidence or catalogue activation.
use std::{collections::BTreeMap, path::Path, sync::Arc, time::Duration};

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
use brassclaw_skills::{
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS, RetainedComponentSnapshot},
    validator_components::{PRELOAD_DEFINITION_SOURCE, preload_definition_draft},
    value_contract::{InputContract, ValueContract},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

async fn source_facts(worker: &Path, source: &str, limits: UtilityLimits) -> Value {
    let UtilityOutput::Inspected { structure } = utility::execute(
        worker,
        UtilityRequest::InspectSource {
            source: source.into(),
            bounds: limits.bounds,
        },
        limits.process,
    )
    .await
    .unwrap() else {
        panic!("actual native source facts required")
    };
    assert_eq!(
        structure.source_checksum,
        hex::encode(Sha256::digest(source.as_bytes()))
    );
    serde_json::to_value(structure).unwrap()
}

fn case(declaration: Value, observations: Value, errors: &[&str]) -> (Value, Value) {
    (
        json!({"declaration":declaration,"observations":observations}),
        json!({"rule_id":"preload_definition","ok":errors.is_empty(),"errors":errors}),
    )
}

#[tokio::test]
async fn packaged_definition_check_retains_exact_draft_and_preserves_operator_successor() {
    use brassclaw_skills::{
        global_bootstrap_components::packaged_component_id,
        revision_store::PgComponentRevisionStore,
    };

    let rig = crate::runtime::test_pg::pg_rig().await;
    crate::builtin_bootstrap::retain_validator_check_drafts(&rig.pool)
        .await
        .unwrap();
    crate::builtin_bootstrap::retain_validator_check_drafts(&rig.pool)
        .await
        .unwrap();
    let id = packaged_component_id("validator:preload-definition");
    let packaged = preload_definition_draft(id).unwrap();
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    assert_eq!(store.read_allocation_head(id).await.unwrap(), Some((22, 1)));
    let original = store.read_revision(id, 1).await.unwrap();
    assert_eq!(original.draft().exact_bytes(), packaged.exact_bytes());
    let mut operator: Value = serde_json::from_str(packaged.exact_bytes()).unwrap();
    operator["document"]["description"] = json!("Operator's unreviewed successor");
    let operator = ComponentRevisionDraft::from_json(&operator.to_string()).unwrap();
    let mut client = rig.pool.get().await.unwrap();
    let tx = client.transaction().await.unwrap();
    let operator_ref = PgComponentRevisionStore::stage_in_transaction(&tx, &operator, 1)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    drop(client);
    crate::builtin_bootstrap::retain_validator_check_drafts(&rig.pool)
        .await
        .unwrap();
    assert_eq!(store.read_allocation_head(id).await.unwrap(), Some((22, 2)));
    assert_eq!(
        store.read_revision(id, 1).await.unwrap().reference(),
        original.reference()
    );
    let retained_operator = store.read_revision(id, 2).await.unwrap();
    assert_eq!(retained_operator.reference(), operator_ref);
    assert_eq!(
        retained_operator.draft().exact_bytes(),
        operator.exact_bytes()
    );
    let client = rig.pool.get().await.unwrap();
    let counts = client
        .query_one(
            "SELECT (SELECT count(*) FROM reborn_component_review_evidence),
                (SELECT count(*) FROM reborn_skill_association_approvals),
                (SELECT count(*) FROM brassclaw_monty_boot_catalogues)",
            &[],
        )
        .await
        .unwrap();
    for column in 0..3 {
        assert_eq!(
            counts.get::<_, i64>(column),
            0,
            "draft retention is not approval or activation"
        );
    }
}

#[tokio::test]
async fn preload_definition_rule_uses_actual_facts_and_rejects_nonempty_invalid_interfaces() {
    let worker = installed_worker().unwrap();
    let limits = UtilityLimits::from_execution(Default::default()).unwrap();
    let draft = preload_definition_draft(Uuid::new_v4()).unwrap();
    let checksum = draft.checksum();
    let inputs =
        InputContract::from_value(&draft.document()["input_contract"], REVISION_LIMITS).unwrap();
    let output =
        ValueContract::result_from_value(&draft.document()["result_contract"], REVISION_LIMITS)
            .unwrap();

    // Der tatsächliche Compiler prüft auch den Prüfer samt privatem Helfer.
    // Dieser private Entwurf erhält dadurch keine Freigabe oder Aktivierung.
    let recipe_id = Uuid::new_v4();
    let recipe = ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1","uuid":recipe_id,"class_code":21,
            "document":{
                "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":[],"variable_patterns":[]}],
                "step_descriptions":[{"desc_idx":0,"label":"Private definition check","yaml_source":"","steps":[
                    {"stepnumber":1,"knowledge":"orchestrator","goal":"Compare definition facts","content":"","type":"component","include":[draft.uuid()]}
                ]}],
                "input_layouts":{"selected":{"format":"recipe-input-layout/1",
                    "task_inputs":draft.document()["input_contract"],"steps":{"0:1":{
                        "declaration":{"kind":"task_input","reference":"{{vars.declaration}}"},
                        "observations":{"kind":"task_input","reference":"{{vars.observations}}"}
                    }}}}
            },"dependencies":[draft.uuid()],"association":null
        }).to_string(),
    ).unwrap();
    let snapshot = Arc::new(
        RetainedComponentSnapshot::new(
            &[recipe_id],
            vec![
                recipe.at_version(1).unwrap(),
                draft.clone().at_version(1).unwrap(),
            ],
        )
        .unwrap(),
    );
    let program = prepare_retained_unbound_program(
        compile_retained_recipe(
            snapshot,
            recipe_id,
            "selected",
            WorkflowClass::Deterministic,
        )
        .unwrap(),
    )
    .unwrap();
    let inspected = InspectedRetainedProgram::inspect_with_limits(
        RetainedProgram::Unbound(Arc::new(program)),
        &worker,
        limits,
    )
    .await
    .unwrap();
    let pure = inspected.pure_component(draft.uuid()).unwrap();
    assert_eq!(pure.components().len(), 1);
    assert_eq!(pure.export_invocations().len(), 1);
    let invocation = inspected.invocation("0:1").unwrap();
    assert_eq!(
        invocation,
        "result = check_preload_definition(inputs=inputs)"
    );
    assert!(
        pure.sources()[&draft.uuid()]
            .observations()
            .direct_host_calls
            .is_empty()
    );
    let source = format!("{PRELOAD_DEFINITION_SOURCE}\n{invocation}\nresult\n");

    let names = [
        "",
        "_helper",
        "public",
        "A09",
        "host",
        "inputs",
        "result",
        "exec",
        "eval",
        "open",
        "__import__",
        "False",
        "None",
        "True",
        "and",
        "as",
        "assert",
        "async",
        "await",
        "break",
        "class",
        "continue",
        "def",
        "del",
        "elif",
        "else",
        "except",
        "finally",
        "for",
        "from",
        "global",
        "if",
        "import",
        "in",
        "is",
        "lambda",
        "nonlocal",
        "not",
        "or",
        "pass",
        "raise",
        "return",
        "try",
        "while",
        "with",
        "yield",
        "ü",
        "a-b",
        "9name",
        "name\n",
        "{{vars.slot0}}",
    ];
    let UtilityOutput::Evaluated { value: identifiers, stdout } = utility::execute(
        &worker,
        UtilityRequest::Evaluate {
            source: format!("{PRELOAD_DEFINITION_SOURCE}\n[_validator_preload_identifier(name) for name in names]\n"),
            inputs: BTreeMap::from([("names".into(), json!(names.as_slice()))]),
            bounds: limits.bounds,
            max_compute_time: Duration::from_secs(5),
        },
        limits.process,
    ).await.unwrap() else {
        panic!("actual identifier rule execution required")
    };
    assert!(stdout.is_empty());
    assert_eq!(
        identifiers,
        json!(
            names
                .map(brassclaw_engine::executor::retained_preload::identifier)
                .as_slice()
        )
    );

    let candidate = "LIMIT = 3\ndef helper(value):\n    import json\n    return value\ndef run(inputs):\n    return helper(inputs['text'])\n";
    let facts = source_facts(&worker, candidate, limits).await;
    let declaration = json!({"format":"python-preload/2","exports":{
        "run":{"symbol":"run","parameters":["inputs"],"mapping":true}},
        "private_functions":{"helper":["value"]},"constants":["LIMIT"],"imports":["json"],"dependencies":[],"default_export":"run"});
    let good = case(declaration.clone(), facts.clone(), &[]);
    let mut cases = vec![good.clone()];
    let mut changed = declaration.clone();
    changed["exports"]["run"]["parameters"] = json!(["text"]);
    cases.push(case(
        changed,
        facts.clone(),
        &[
            "preload.exports: mapping export requires exactly inputs",
            "source.preload.functions: declared signatures differ from parsed definitions",
        ],
    ));
    let mut changed = declaration.clone();
    changed["private_functions"]["helper"] = json!(["other"]);
    cases.push(case(
        changed,
        facts.clone(),
        &["source.preload.functions: declared signatures differ from parsed definitions"],
    ));
    let mut changed = declaration.clone();
    changed["constants"] = json!([]);
    cases.push(case(
        changed,
        facts.clone(),
        &["source.preload.constants: declarations differ from parsed definitions"],
    ));
    let mut changed = declaration.clone();
    changed["imports"] = json!([]);
    cases.push(case(
        changed,
        facts.clone(),
        &["source.imports: declarations differ from parsed modules"],
    ));
    let mut changed = declaration.clone();
    changed["imports"] = json!(["json", "json"]);
    cases.push(case(
        changed,
        facts.clone(),
        &["preload.imports: duplicate module"],
    ));
    let mut changed = declaration.clone();
    changed["default_export"] = json!("missing");
    cases.push(case(
        changed,
        facts.clone(),
        &["preload.default_export: unknown public export"],
    ));
    let mut changed = declaration.clone();
    changed["format"] = json!("python-preload/unknown");
    cases.push(case(
        changed,
        facts.clone(),
        &["preload.format: unsupported declaration"],
    ));
    let mut changed = declaration.clone();
    changed["exports"]["_run"] = changed["exports"]["run"].take();
    changed["exports"].as_object_mut().unwrap().remove("run");
    changed["default_export"] = json!("_run");
    cases.push(case(
        changed,
        facts.clone(),
        &["preload.exports: invalid public name"],
    ));
    let mut changed = declaration.clone();
    changed["exports"]["alias"] = changed["exports"]["run"].clone();
    cases.push(case(
        changed,
        facts.clone(),
        &["preload.exports: invalid or conflicting implementation symbol"],
    ));
    let mut changed = declaration.clone();
    changed["constants"] = json!(["LIMIT", "LIMIT"]);
    cases.push(case(
        changed,
        facts.clone(),
        &["preload.constants: invalid or conflicting symbol"],
    ));
    cases.push(case(
        Value::Null,
        facts.clone(),
        &["preload: declaration is missing"],
    ));
    let mut changed = declaration.clone();
    changed.as_object_mut().unwrap().remove("default_export");
    cases.push(case(changed, facts.clone(), &[]));

    // Nichtleere, syntaktisch gültige Kandidaten dürfen keine falschen
    // Erfolgsmeldungen erhalten. Ihre Effekte werden niemals ausgeführt.
    for candidate in [
        candidate.replace(
            "def run(inputs):",
            "def run(inputs=host.post_reply(answer='not executed')):",
        ),
        format!("{candidate}\nCACHE = []\n"),
        format!("{candidate}\nhost.post_reply(answer='not executed')\n"),
    ] {
        let observed = source_facts(&worker, &candidate, limits).await;
        assert!(observed["preload"].is_null());
        cases.push(case(declaration.clone(), observed, &[
            "source.preload: definitions cannot be loaded without effects under the selected profile",
        ]));
    }
    let candidate_with_extra = format!("{candidate}\ndef undeclared(value):\n    return value\n");
    cases.push(case(
        declaration.clone(),
        source_facts(&worker, &candidate_with_extra, limits).await,
        &["source.preload.functions: declared signatures differ from parsed definitions"],
    ));

    // Eine echte Tool-Nutzung ist kein reiner Helfer. Diese Definitionsregel
    // prüft absichtlich keine Aufruf- oder Assoziationsfreigabe.
    let tool_facts = source_facts(
        &worker,
        "def use_tool(inputs):\n    return host.post_reply(answer=inputs['text'])\n",
        limits,
    )
    .await;
    assert_eq!(tool_facts["direct_host_calls"].as_array().unwrap().len(), 1);
    cases.push(case(json!({"format":"python-preload/2","exports":{
        "use_tool":{"symbol":"use_tool","parameters":["inputs"],"mapping":true}},
        "private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"use_tool"}), tool_facts, &[]));
    let helper_facts = source_facts(
        &worker,
        "def private_helper(value):\n    return value\n",
        limits,
    )
    .await;
    cases.push(case(json!({"format":"python-preload/2","exports":{},
        "private_functions":{"private_helper":["value"]},"constants":[],"imports":[],"dependencies":[],"default_export":null}), helper_facts, &[]));
    let direct_facts = source_facts(&worker, "def direct(text):\n    return text\n", limits).await;
    cases.push(case(json!({"format":"python-preload/2","exports":{
        "direct":{"symbol":"direct","parameters":["text"],"mapping":false}},
        "private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"direct"}), direct_facts, &[]));
    cases.push(good.clone());

    for (value, expected) in cases {
        let bound = inspected
            .program()
            .inputs()
            .bind_task_inputs(&value)
            .unwrap();
        assert_eq!(bound, Value::Object(inputs.bind(&value).unwrap()));
        let UtilityOutput::Evaluated {
            value: actual,
            stdout,
        } = utility::execute(
            &worker,
            UtilityRequest::Evaluate {
                source: source.clone(),
                inputs: BTreeMap::from([("inputs".into(), bound)]),
                bounds: limits.bounds,
                max_compute_time: Duration::from_secs(5),
            },
            limits.process,
        )
        .await
        .unwrap()
        else {
            panic!("actual pure invocation required")
        };
        assert!(stdout.is_empty());
        output.validate(&actual).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(draft.checksum(), checksum);
    }
    for value in [
        json!({"declaration":declaration}),
        json!({"declaration":declaration,"observations":null}),
        json!({"declaration":declaration,"observations":facts,"passed":true}),
    ] {
        assert!(inputs.bind(&value).is_err());
    }
    let mut forged = good.0;
    forged["observations"]["approved"] = json!(true);
    assert!(inputs.bind(&forged).is_err());
    forged["observations"]
        .as_object_mut()
        .unwrap()
        .remove("approved");
    forged["declaration"]["exports"]["run"]["mapping"] = json!(null);
    assert!(inputs.bind(&forged).is_err());
}
