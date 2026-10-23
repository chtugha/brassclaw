//! Actual contained Monty execution of a pure checking component. This does not
//! establish a complete class validator or successful authored Q1 evidence.
use brassclaw_monty_host::{
    process::installed_worker,
    utility::{self, UtilityLimits, UtilityOutput, UtilityRequest},
};
use brassclaw_skills::{
    component_revision::REVISION_LIMITS,
    validator_components::{AGGREGATE_SOURCE, COLLECT_SOURCE, aggregate_draft, collect_draft},
    value_contract::{InputContract, ValueContract},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use uuid::Uuid;

use crate::pg_association_review::tests::native_evidence::support as observation_runtime;

async fn observation_exchange(
    transport: &brassclaw_monty_host::transport_actor::TransportClient,
    command: brassclaw_monty_host::process::WorkerCommand,
) -> brassclaw_monty_host::process::ProcessSnapshot {
    transport
        .try_submit(command)
        .unwrap()
        .wait()
        .await
        .unwrap()
        .outcome
        .unwrap()
}

#[tokio::test]
async fn pure_review_source_closure_checks_unselected_exports_and_dependencies() {
    use brassclaw_engine::{
        executor::{
            retained_recipe::RetainedProgram,
            retained_source::{InspectedRetainedProgram, RetainedSourceError},
        },
        memory::{
            retained_inputs::prepare_retained_unbound_program,
            retained_instruction::{WorkflowClass, compile_retained_recipe},
        },
    };
    use brassclaw_skills::component_revision::{ComponentRevisionDraft, RetainedComponentSnapshot};

    // These exact draft graphs exercise the real parser/preload inspector.
    // No approval, component activation or successful-Q1 row is manufactured.
    async fn inspect(
        root: Uuid,
        helper: Uuid,
        document: Value,
        helper_document: Option<Value>,
    ) -> InspectedRetainedProgram {
        let recipe = Uuid::new_v4();
        let mut drafts = vec![
            ComponentRevisionDraft::from_json(&json!({
                "format":"component-revision/1","uuid":recipe,"class_code":21,
                "document":{
                    "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":[],"variable_patterns":[]}],
                    "step_descriptions":[{"desc_idx":0,"label":"Private source inspection","yaml_source":"","steps":[
                        {"stepnumber":1,"knowledge":"orchestrator","goal":"Inspect pure source","content":"","type":"component","include":[root]}
                    ]}],
                    "input_layouts":{"selected":{"format":"recipe-input-layout/1",
                        "task_inputs":{"text":{"type":"string","required":true,"checks":[]}},
                        "steps":{"0:1":{"text":{"kind":"task_input","reference":"{{vars.text}}"}}}}}
                },"dependencies":[root],"association":null
            }).to_string()).unwrap(),
            ComponentRevisionDraft::from_json(&json!({
                "format":"component-revision/1","uuid":root,"class_code":22,
                "document":document,"dependencies":if helper_document.is_some() { vec![helper] } else { vec![] },"association":null
            }).to_string()).unwrap(),
        ];
        if let Some(document) = helper_document {
            drafts.push(
                ComponentRevisionDraft::from_json(
                    &json!({
                        "format":"component-revision/1","uuid":helper,"class_code":22,
                        "document":document,"dependencies":[],"association":null
                    })
                    .to_string(),
                )
                .unwrap(),
            );
        }
        let snapshot = Arc::new(
            RetainedComponentSnapshot::new(
                &[recipe],
                drafts
                    .into_iter()
                    .map(|draft| draft.at_version(1).unwrap())
                    .collect(),
            )
            .unwrap(),
        );
        let program = prepare_retained_unbound_program(
            compile_retained_recipe(snapshot, recipe, "selected", WorkflowClass::Deterministic)
                .unwrap(),
        )
        .unwrap();
        InspectedRetainedProgram::inspect(
            RetainedProgram::Unbound(Arc::new(program)),
            &installed_worker().unwrap(),
        )
        .await
        .unwrap()
    }

    let root = Uuid::new_v4();
    let helper = Uuid::new_v4();
    let helper_document = json!({
        "content":"def identity(value):\n    return value\n",
        "preload":{"format":"python-preload/2","exports":{},"private_functions":{"identity":["value"]},
            "constants":[],"imports":[],"dependencies":[],"default_export":null}
    });
    let source = "def text_identity(inputs):\n    return identity(inputs['text'])\ndef direct_identity(text):\n    return identity(text)\n";
    let document = json!({
        "content":source,"input_contract":{"text":{"type":"string","required":true,"checks":[]}},
        "result_contract":{"type":"string"},
        "preload":{"format":"python-preload/2","exports":{
            "text_identity":{"symbol":"text_identity","parameters":["inputs"],"mapping":true},
            "direct_identity":{"symbol":"direct_identity","parameters":["text"],"mapping":false}
        },"private_functions":{},"constants":[],"imports":[],"dependencies":[helper],"default_export":"text_identity"}
    });
    let inspected = inspect(
        root,
        helper,
        document.clone(),
        Some(helper_document.clone()),
    )
    .await;
    let pure = inspected.pure_component(root).unwrap();
    assert_eq!(pure.component().uuid, root);
    assert_eq!(pure.components().keys().copied().collect::<Vec<_>>(), {
        let mut ids = vec![root, helper];
        ids.sort();
        ids
    });
    for (id, source) in pure.sources() {
        assert_eq!(source.component(), pure.components()[id]);
        assert!(source.observations().direct_host_calls.is_empty());
    }
    assert_eq!(
        pure.export_invocations(),
        &BTreeMap::from([
            (
                "text_identity".into(),
                "result = text_identity(inputs=inputs)".into()
            ),
            (
                "direct_identity".into(),
                "result = direct_identity(text=inputs['text'])".into()
            ),
        ])
    );
    assert!(inspected.pure_component(Uuid::new_v4()).is_err());

    let mut unselected_host = document.clone();
    unselected_host["content"] = json!(source.replace(
        "return identity(text)",
        "return host.unbound_effect(value=text)"
    ));
    let mut hidden_dependency = helper_document.clone();
    hidden_dependency["content"] = json!(
        "def identity(value):\n    return value\ndef hidden_effect(value):\n    return host.unbound_effect(value=value)\n"
    );
    hidden_dependency["preload"]["private_functions"]["hidden_effect"] = json!(["value"]);
    let mut wrong_signature = document.clone();
    wrong_signature["content"] = json!(source.replace(
        "direct_identity(text):\n    return identity(text)",
        "direct_identity(other):\n    return identity(other)"
    ));
    wrong_signature["preload"]["exports"]["direct_identity"]["parameters"] = json!(["other"]);
    let mut recursive_export = document.clone();
    recursive_export["content"] =
        json!(source.replace("return identity(text)", "return direct_identity(text)"));
    let mut legacy = document.clone();
    legacy["content"] = json!("result = inputs['text']");
    legacy.as_object_mut().unwrap().remove("preload");
    for (candidate, dependency, rejected_component, reason) in [
        (
            unselected_host,
            Some(helper_document.clone()),
            root,
            "pure closure contains a legacy body or host call",
        ),
        (
            document,
            Some(hidden_dependency),
            helper,
            "pure closure contains a legacy body or host call",
        ),
        (
            wrong_signature,
            Some(helper_document.clone()),
            root,
            "export signature differs from input contract",
        ),
        (
            recursive_export,
            Some(helper_document),
            root,
            "recursive function graph requires unsupported invocation qualification",
        ),
        (
            legacy,
            None,
            root,
            "pure root requires declared preload exports",
        ),
    ] {
        // Ordinary inspection has selected text_identity only. That cannot
        // certify every export/dependency as an independently pure component.
        let inspected = inspect(root, helper, candidate, dependency).await;
        let Err(RetainedSourceError::Invalid {
            component,
            reason: actual,
        }) = inspected.pure_component(root)
        else {
            panic!("the complete pure subject must reject {reason}")
        };
        assert_eq!(component, rejected_component);
        assert_eq!(actual, reason);
    }
}

#[tokio::test]
async fn typed_structural_successor_keeps_hostile_data_out_of_unchanged_source() {
    use brassclaw_skills::validator_components::{
        STRUCTURAL_PRESENCE_SOURCE, structural_presence_draft,
    };
    use sha2::{Digest, Sha256};
    let draft = structural_presence_draft(Uuid::new_v4()).unwrap();
    let checksum = draft.checksum();
    let inputs =
        InputContract::from_value(&draft.document()["input_contract"], REVISION_LIMITS).unwrap();
    let output =
        ValueContract::result_from_value(&draft.document()["result_contract"], REVISION_LIMITS)
            .unwrap();
    let limits =
        UtilityLimits::from_execution(brassclaw_host_api::MontyExecutionLimits::default()).unwrap();
    let worker = installed_worker().unwrap();
    let UtilityOutput::Inspected { structure } = utility::execute(
        &worker,
        UtilityRequest::InspectSource {
            source: STRUCTURAL_PRESENCE_SOURCE.into(),
            bounds: limits.bounds,
        },
        limits.process,
    )
    .await
    .unwrap() else {
        panic!("actual source inspection required")
    };
    assert_eq!(
        structure.source_checksum,
        hex::encode(Sha256::digest(STRUCTURAL_PRESENCE_SOURCE.as_bytes()))
    );
    assert!(structure.imports.is_empty());
    assert!(structure.direct_host_calls.is_empty());
    assert_eq!(structure.host_value_references, 0);
    let hostile = json!({"name":"' ü\n{{vars.slot0}}", "description":"{inputs} \"\\", "content":"raise RuntimeError('must remain data')\n{{vars.slot2}}"});
    for (value, expected) in [
        (
            hostile.clone(),
            json!({"rule_id":"structural_presence","ok":true,"errors":[]}),
        ),
        (
            json!({"name":" \n", "description":"\t", "content":""}),
            json!({"rule_id":"structural_presence","ok":false,"errors":["name is empty","description is empty","content is empty"]}),
        ),
        (
            hostile,
            json!({"rule_id":"structural_presence","ok":true,"errors":[]}),
        ),
    ] {
        inputs.bind(&value).unwrap();
        let UtilityOutput::Evaluated {
            value: actual,
            stdout,
        } = utility::execute(
            &worker,
            UtilityRequest::Evaluate {
                source: format!(
                    "{STRUCTURAL_PRESENCE_SOURCE}\ncheck_structural_presence(inputs)\n"
                ),
                inputs: BTreeMap::from([("inputs".into(), value)]),
                bounds: limits.bounds,
                max_compute_time: Duration::from_secs(5),
            },
            limits.process,
        )
        .await
        .unwrap()
        else {
            panic!("actual typed execution required")
        };
        assert!(stdout.is_empty());
        output.validate(&actual).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(draft.checksum(), checksum);
    }
    assert!(
        inputs
            .bind(&json!({"name":null,"description":"d","content":"c"}))
            .is_err()
    );
    assert!(inputs.bind(&json!({"name":"n","description":"d"})).is_err());
    assert!(
        inputs
            .bind(&json!({"name":"n","description":"d","content":"c","pass":true}))
            .is_err()
    );
}

#[tokio::test]
async fn retained_validator_coverage_requires_private_exact_steps_and_pinned_rules() {
    use brassclaw_engine::memory::{
        retained_inputs::prepare_retained_unbound_program,
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_validator::RetainedValidatorCoverage,
    };
    use brassclaw_skills::component_revision::{ComponentRevisionDraft, RetainedComponentSnapshot};
    let (root, rule_id) = (Uuid::new_v4(), Uuid::new_v4());
    let aggregate = aggregate_draft(Uuid::new_v4()).unwrap();
    let rule=ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":rule_id,"class_code":22,
        "document":{"content":"def inspect_identity(inputs):\n    if inputs['subject'] == 'large diagnostics':\n        return {'rule_id': 'identity', 'ok': False, 'errors': ['observed diagnostic'] * 300000}\n    return {'rule_id': 'identity', 'ok': inputs['subject'] != '', 'errors': [] if inputs['subject'] != '' else ['subject missing']}",
            "input_contract":{"subject":{"type":"string","required":true,"checks":[]}},
            "result_contract":{"type":"object","allow_extra_fields":false,"fields":{
                "rule_id":{"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]},"ok":{"type":"boolean","required":true},
                "errors":{"type":"list","required":true,"items":{"type":"string","checks":[{"kind":"min_length","value":1}]}}}},
            "preload":{"format":"python-preload/2","exports":{"inspect_identity":{"symbol":"inspect_identity","parameters":["inputs"],"mapping":true}},
                "private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"inspect_identity"}},
        "dependencies":[],"association":null}).to_string()).unwrap();
    let collector_id = Uuid::new_v4();
    let collector = collect_draft(collector_id, &["identity".into()]).unwrap();
    for names in [
        vec![],
        vec!["identity".into(), "identity".into()],
        vec!["Identity".into()],
    ] {
        assert!(collect_draft(Uuid::new_v4(), &names).is_err());
    }
    // This is only a preparation fixture, not a complete identity rule/validator
    // or fabricated successful execution/approval observation.
    let base = json!({"validates_class_code":22,"consumer_tags":["05:validator"],
    "validator_coverage":{"format":"validator-coverage/1","rules":[{"rule_id":"identity","step_id":"0:1"}],"aggregate_step":"0:3"},
    "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":[],"variable_patterns":[]}],
    "step_descriptions":[{"desc_idx":0,"label":"coverage preparation","yaml_source":"","steps":[
        {"stepnumber":1,"knowledge":"orchestrator","goal":"Observe rule","content":"","type":"component","include":[rule_id]},
        {"stepnumber":2,"knowledge":"orchestrator","goal":"Collect rule results","content":"","type":"component","include":[collector_id]},
        {"stepnumber":3,"knowledge":"orchestrator","goal":"Aggregate rules","content":"","type":"component","include":[aggregate.uuid()]}
    ]}],"input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":{
        "subject":{"type":"string","required":true,"checks":[]}},
        "steps":{"0:1":{"subject":{"kind":"task_input","reference":"{{vars.subject}}"}},"0:2":{"identity":{"kind":"result","step_id":"0:1","path":[]}},"0:3":{
            "checks":{"kind":"result","step_id":"0:2","path":[]},"required_rule_ids":{"kind":"constant","value":["identity"]}
        }}}}});
    let prepare_with_collector = |document: Value, collector: &ComponentRevisionDraft| {
        let recipe = ComponentRevisionDraft::from_json(
            &json!({"format":"component-revision/1","uuid":root,"class_code":21,
            "document":document,"dependencies":[rule_id,collector_id,aggregate.uuid()],"association":null})
            .to_string(),
        )
        .unwrap();
        let graph = Arc::new(
            RetainedComponentSnapshot::new(
                &[root],
                vec![
                    recipe.at_version(1).unwrap(),
                    rule.clone().at_version(1).unwrap(),
                    aggregate.clone().at_version(1).unwrap(),
                    collector.clone().at_version(1).unwrap(),
                ],
            )
            .unwrap(),
        );
        prepare_retained_unbound_program(
            compile_retained_recipe(graph, root, "selected", WorkflowClass::Deterministic).unwrap(),
        )
        .unwrap()
    };
    let prepare = |document| prepare_with_collector(document, &collector);
    let program = prepare(base.clone());
    let coverage = RetainedValidatorCoverage::from_inputs(program.inputs()).unwrap();
    assert_eq!(coverage.class_code(), 22);
    assert_eq!(
        coverage.rules(),
        &BTreeMap::from([("identity".into(), "0:1".into())])
    );
    assert_eq!(coverage.aggregate_step(), "0:3");
    RetainedValidatorCoverage::validate_catalogue([program.inputs()]).unwrap();
    assert_eq!(
        RetainedValidatorCoverage::validate_catalogue([program.inputs(), program.inputs()]).err(),
        Some("ambiguous validator class route"),
        "two selected variants or Recipes cannot choose the validator by iteration order"
    );
    let mut other = base.clone();
    other["validates_class_code"] = json!(21);
    let other_program = prepare(other);
    RetainedValidatorCoverage::validate_catalogue([program.inputs(), other_program.inputs()])
        .unwrap();
    let mut untagged = base.clone();
    untagged.as_object_mut().unwrap().remove("consumer_tags");
    let untagged_program = prepare(untagged);
    assert!(RetainedValidatorCoverage::validate_catalogue([untagged_program.inputs()]).is_err());
    let mut ordinary = base.clone();
    for field in [
        "consumer_tags",
        "validates_class_code",
        "validator_coverage",
    ] {
        ordinary.as_object_mut().unwrap().remove(field);
    }
    let ordinary_program = prepare(ordinary);
    RetainedValidatorCoverage::validate_catalogue([ordinary_program.inputs()]).unwrap();
    let mut invalid = Vec::new();
    let mut value = base.clone();
    value["consumer_tags"] = json!(["05:validator", "02:orchestrator"]);
    invalid.push(value);
    let mut value = base.clone();
    value["validates_class_code"] = json!(50);
    invalid.push(value);
    let mut value = base.clone();
    value["validator_coverage"]["rules"] = json!([]);
    invalid.push(value);
    let mut value = base.clone();
    value["validator_coverage"]["rules"]
        .as_array_mut()
        .unwrap()
        .push(json!({"rule_id":"identity","step_id":"0:1"}));
    invalid.push(value);
    let mut value = base.clone();
    value["validator_coverage"]["rules"][0]["step_id"] = json!("0:9");
    invalid.push(value);
    let mut value = base.clone();
    value["validator_coverage"]["rules"][0]["step_id"] = json!("0:2");
    invalid.push(value);
    let mut value = base.clone();
    value["validator_coverage"]["aggregate_step"] = json!("0:9");
    invalid.push(value);
    let mut value = base.clone();
    value["validator_coverage"]["passed"] = json!(true);
    invalid.push(value);
    let mut value = base.clone();
    value["input_layouts"]["selected"]["steps"]["0:3"]["required_rule_ids"]["value"] =
        json!(["other"]);
    invalid.push(value);
    let mut value = base.clone();
    value["variants"][0]["intent_examples"] = json!(["public validator"]);
    invalid.push(value);
    let mut value = base.clone();
    value["input_layouts"]["selected"]["task_inputs"]["checks"] =
        aggregate.document()["input_contract"]["checks"].clone();
    value["input_layouts"]["selected"]["steps"]["0:3"]["checks"] =
        json!({"kind":"task_input","reference":"{{vars.checks}}"});
    invalid.push(value);
    let mut renamed = base.clone();
    renamed["input_layouts"]["selected"]["steps"]["0:2"] =
        json!({"other":{"kind":"result","step_id":"0:1","path":[]}});
    let renamed_collector = collect_draft(collector_id, &["other".into()]).unwrap();
    let renamed_program = prepare_with_collector(renamed, &renamed_collector);
    assert!(
        RetainedValidatorCoverage::from_inputs(renamed_program.inputs()).is_err(),
        "typed collection bindings still must preserve the pinned rule-to-step identity"
    );
    let mut value = base.clone();
    value["input_layouts"]["selected"]["steps"]["0:2"]["identity"] =
        json!({"kind":"constant","value":{"rule_id":"identity","ok":true,"errors":[]}});
    invalid.push(value);
    for document in invalid {
        assert!(RetainedValidatorCoverage::from_inputs(prepare(document).inputs()).is_err());
    }

    // Real constrained draft execution, not an approved class validator or
    // fabricated Q1 pass. A live root retains the task while child feeds run.
    use brassclaw_engine::executor::{
        retained_recipe::{RetainedProgram, RetainedRecipeExecution, RetainedTaskContext},
        retained_source::InspectedRetainedProgram,
    };
    use brassclaw_monty_host::{
        heap::HeapSettings,
        process::{ProcessBoundary, RecipeCommand, TaskSettings, WorkerCommand},
        transport_actor::{ActorLimits, TransportOwner},
    };
    let source = "import asyncio\nasync def observe():\n    task = await host.await_next_task(0)\n    host.enter_task(task['task_token'])\n    await host.validation_hold(task['task_token'])\nasyncio.run(observe())\n";
    let mut boot = observation_runtime::boot(source);
    boot.bounds.workers = 1;
    boot.aliases.insert("validation_hold".into());
    boot.heap_settings = Some(HeapSettings {
        revision: 1,
        max_vm_bytes: 512 * 1024 * 1024,
    });
    boot.bounds.values.max_value_nodes = 1_048_576;
    boot.bounds.values.max_value_bytes = 16 * 1024 * 1024;
    let admitted_bounds = boot.bounds.values;
    let mut transport_limits = observation_runtime::limits();
    transport_limits.hard_memory_bytes = 1024 * 1024 * 1024;
    transport_limits.max_frame_bytes = 32 * 1024 * 1024;
    transport_limits.response_timeout = Duration::from_secs(30);
    let (mut owner, ready) = TransportOwner::start(
        observation_runtime::worker(),
        boot,
        transport_limits,
        ActorLimits {
            max_unclaimed: 8,
            max_reserved_frame_bytes: 64 * 1024 * 1024,
            max_control_unclaimed: 8,
            max_control_reserved_frame_bytes: 64 * 1024 * 1024,
        },
    )
    .await
    .unwrap();
    let transport = owner.client();
    let mut root_progress = observation_exchange(&transport, WorkerCommand::Admit {
        key: ready.work_waits[0].1,
        task: json!({"conversation_id":"observation-fixture","message_id":"observation-message",
            "run_id":"observation-run","turn_id":"observation-turn","user_input":"typed subject","history":[]}),
    }).await;
    let task = root_progress.admitted_task.unwrap();
    while let Some(ProcessBoundary::ControlYield { key }) = root_progress.boundary {
        root_progress =
            observation_exchange(&transport, WorkerCommand::ResumeControl { key }).await;
    }
    let Some(ProcessBoundary::HostCall { name, key, .. }) = root_progress.boundary else {
        panic!("real root wait required")
    };
    assert_eq!(name, "validation_hold");
    observation_exchange(&transport, WorkerCommand::Defer { key }).await;
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect(
            RetainedProgram::Unbound(Arc::new(program)),
            observation_runtime::worker(),
        )
        .await
        .unwrap(),
    );
    let mut parent = RetainedTaskContext::new(task);
    let parent = parent.open(&transport).await.unwrap();
    let mut execution = RetainedRecipeExecution::new_child_for_behavioral_validation(
        task,
        parent,
        inspected.clone(),
    )
    .unwrap();
    let mut results = BTreeMap::new();
    assert!(
        coverage
            .observe(&execution, &results, REVISION_LIMITS)
            .is_err()
    );
    for (step, inputs) in [
        ("0:1", json!({"subject":"complete typed data"})),
        (
            "0:2",
            json!({"identity":{"rule_id":"identity","ok":true,"errors":[]}}),
        ),
        (
            "0:3",
            json!({"required_rule_ids":["identity"],"checks":[{"rule_id":"identity","ok":true,"errors":[]}]}),
        ),
    ] {
        let value = execution
            .run_step(&transport, step, &inputs, None)
            .await
            .unwrap();
        results.insert(step.to_owned(), value);
    }
    let observed = coverage
        .observe(&execution, &results, REVISION_LIMITS)
        .unwrap();
    assert_eq!(
        observed.report()["aggregation"],
        json!({"pass":true,"errors":[]})
    );
    assert_eq!(observed.report()["q1_qualification"], false);
    assert_eq!(observed.report()["semantic_approval"], false);
    let mut swapped = results.clone();
    swapped.get_mut("0:1").unwrap()["ok"] = json!(false);
    assert!(
        coverage
            .observe(&execution, &swapped, REVISION_LIMITS)
            .is_err()
    );
    let mut missing = results.clone();
    missing.remove("0:2");
    assert!(
        coverage
            .observe(&execution, &missing, REVISION_LIMITS)
            .is_err()
    );
    let unobserved = RetainedRecipeExecution::new_child(task, parent, inspected.clone()).unwrap();
    assert!(
        coverage
            .observe(&unobserved, &results, REVISION_LIMITS)
            .is_err()
    );
    assert!(
        RetainedValidatorCoverage::from_inputs(other_program.inputs())
            .unwrap()
            .observe(&execution, &results, REVISION_LIMITS)
            .is_err()
    );
    let small = brassclaw_skills::value_contract::ContractLimits {
        max_bytes: 1,
        ..REVISION_LIMITS
    };
    assert!(coverage.observe(&execution, &results, small).is_err());

    // Actual larger results must use their captured per-step limits, rather
    // than the old default node ceiling or later settings. Preserve the whole
    // negative candidate result; this is observation, not successful Q1.
    let mut large_execution = RetainedRecipeExecution::new_child_for_behavioral_validation(
        task,
        parent,
        inspected.clone(),
    )
    .unwrap();
    let mut large_results: BTreeMap<String, Value> = BTreeMap::new();
    let mut later_bounds = admitted_bounds;
    later_bounds.max_value_nodes = 600_000;
    for step in ["0:1", "0:2", "0:3"] {
        if step == "0:2" {
            let updated = observation_exchange(
                &transport,
                WorkerCommand::Recipe {
                    command: RecipeCommand::UpdateRuntimeSettings {
                        expected_revision: 1,
                        settings: TaskSettings {
                            revision: 2,
                            max_compute_time: Duration::from_secs(600),
                            token_budgets_enabled: false,
                        },
                        values: later_bounds,
                        max_pending_host_calls: None,
                        max_workers: None,
                        max_recipe_contexts: None,
                        adapter_reserve_bytes: None,
                        heap_update: None,
                        frame_update: None,
                    },
                },
            )
            .await;
            assert_eq!(updated.root, ready.root);
            assert_eq!(*transport.live_vm_bounds().borrow(), later_bounds);
        }
        let inputs = match step {
            "0:1" => json!({"subject":"large diagnostics"}),
            "0:2" => json!({"identity":large_results["0:1"]}),
            "0:3" => json!({"required_rule_ids":["identity"],"checks":large_results["0:2"]}),
            _ => unreachable!(),
        };
        let value = large_execution
            .run_step(&transport, step, &inputs, None)
            .await
            .unwrap();
        large_results.insert(step.to_owned(), value);
    }
    assert!(large_execution.is_complete());
    assert!(large_execution.host_answers().is_empty());
    assert_eq!(
        large_results["0:1"]["errors"].as_array().unwrap().len(),
        300_000
    );
    assert!(
        brassclaw_engine::executor::retained_recipe::typed_value_checksum(&large_results["0:1"])
            .is_err(),
        "the actual output must exceed the historical default node ceiling"
    );
    assert_eq!(
        large_execution.observations()["0:1"]
            .data_limits()
            .max_nodes,
        admitted_bounds.max_value_nodes
    );
    assert_eq!(
        large_execution.observations()["0:2"]
            .data_limits()
            .max_nodes,
        later_bounds.max_value_nodes
    );
    let mut smaller_bounds = later_bounds;
    smaller_bounds.max_value_nodes = 2048;
    smaller_bounds.max_value_bytes = 16384;
    let updated = observation_exchange(
        &transport,
        WorkerCommand::Recipe {
            command: RecipeCommand::UpdateRuntimeSettings {
                expected_revision: 2,
                settings: TaskSettings {
                    revision: 3,
                    max_compute_time: Duration::from_secs(600),
                    token_budgets_enabled: false,
                },
                values: smaller_bounds,
                max_pending_host_calls: None,
                max_workers: None,
                max_recipe_contexts: None,
                adapter_reserve_bytes: None,
                heap_update: None,
                frame_update: None,
            },
        },
    )
    .await;
    assert_eq!(updated.root, ready.root);
    assert_eq!(*transport.live_vm_bounds().borrow(), smaller_bounds);
    let report_limits = brassclaw_skills::value_contract::ContractLimits {
        max_nodes: admitted_bounds.max_value_nodes,
        max_bytes: admitted_bounds.max_value_bytes,
        ..REVISION_LIMITS
    };
    let complete = coverage
        .observe(&large_execution, &large_results, report_limits)
        .unwrap();
    assert_eq!(complete.report()["aggregation"]["pass"], false);
    assert_eq!(
        complete.report()["aggregation"]["errors"][0]["errors"]
            .as_array()
            .unwrap()
            .len(),
        300_000
    );
    assert_eq!(complete.report()["q1_qualification"], false);
    assert!(
        coverage
            .observe(&large_execution, &large_results, REVISION_LIMITS)
            .is_err(),
        "separate report bounds still apply; they must not truncate diagnostics"
    );
    large_results.get_mut("0:1").unwrap()["errors"][0] = json!("changed diagnostic");
    assert!(
        coverage
            .observe(&large_execution, &large_results, report_limits)
            .is_err(),
        "larger admitted limits must not accept changed execution data"
    );
    owner.request_termination();
    owner.join().await.unwrap();

    // Persist only the constrained fixture's preparation selection. This
    // neither activates the fixture nor produces successful Q1/approval data.
    use brassclaw_skills::{
        review_submission_store::{
            PgReviewSubmissionStore, ReviewSubmissionDraft, SubmissionStoreError, selection,
        },
        revision_store::PgComponentRevisionStore,
    };
    let rig = crate::runtime::test_pg::pg_rig().await;
    let revisions = PgComponentRevisionStore::new(rig.pool.clone());
    for revision in inspected
        .program()
        .inputs()
        .instruction()
        .snapshot()
        .revisions()
        .values()
    {
        let staged = revisions.stage(revision.draft(), 0).await.unwrap();
        assert_eq!(staged, revision.reference());
    }
    let subject_id = Uuid::new_v4();
    let candidate =
        brassclaw_skills::validator_components::structural_presence_draft(Uuid::new_v4()).unwrap();
    PgReviewSubmissionStore::new(rig.pool.clone())
        .submit(
            &ReviewSubmissionDraft::new(subject_id, "preparation-fixture", candidate, None, vec![])
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
    let subject = PgReviewSubmissionStore::read_validator_subject_in_transaction(
        &tx,
        subject_id,
        REVISION_LIMITS,
    )
    .await
    .unwrap()
    .unwrap();
    let bytes =
        RetainedValidatorCoverage::selection_bytes(&subject, &inspected, REVISION_LIMITS).unwrap();
    assert!(RetainedValidatorCoverage::selection_bytes(&subject, &inspected, small).is_err());
    selection::retain_in_transaction(&tx, &bytes, REVISION_LIMITS)
        .await
        .unwrap();
    let saved = selection::read_preparation_in_transaction(&tx, subject_id, REVISION_LIMITS)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(subject.submission_id(), subject_id);
    assert_eq!(saved.submission_id(), subject.submission_id());
    assert_eq!(saved.subject_checksum(), subject.subject_checksum());
    let restored = RetainedValidatorCoverage::restore_preparation(&saved).unwrap();
    assert_eq!(
        restored
            .inputs()
            .instruction()
            .retained_selection()
            .unwrap()
            .exact_bytes(),
        inspected
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .unwrap()
            .exact_bytes()
    );
    selection::retain_in_transaction(&tx, &bytes, REVISION_LIMITS)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let view = crate::pg_review_submission::get(&rig.pool, &subject_id.to_string())
        .await
        .unwrap();
    assert_eq!(view.review_selection_bytes.as_deref(), Some(bytes.as_str()));
    assert_eq!(view.receipt.review_status, "unreviewed");
    assert!(!view.receipt.catalogue_activated);
    let root_revision = &inspected
        .program()
        .inputs()
        .instruction()
        .snapshot()
        .revisions()[&root];
    let mut successor: Value = serde_json::from_str(root_revision.draft().exact_bytes()).unwrap();
    successor["document"]["description"] =
        json!("Later draft must not replace a retained selection");
    revisions
        .stage(
            &ComponentRevisionDraft::from_json(&successor.to_string()).unwrap(),
            1,
        )
        .await
        .unwrap();
    assert_eq!(
        crate::pg_review_submission::get(&rig.pool, &subject_id.to_string())
            .await
            .unwrap()
            .review_selection_bytes
            .as_deref(),
        Some(bytes.as_str())
    );
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let mut changed: Value = serde_json::from_str(&bytes).unwrap();
    changed["validator_selection_bytes"] = json!(
        changed["validator_selection_bytes"]
            .as_str()
            .unwrap()
            .to_owned()
            + " "
    );
    let changed_checksum = {
        use sha2::{Digest, Sha256};
        format!(
            "{:x}",
            Sha256::digest(
                changed["validator_selection_bytes"]
                    .as_str()
                    .unwrap()
                    .as_bytes()
            )
        )
    };
    changed["validator_selection_checksum"] = json!(changed_checksum);
    assert!(matches!(
        selection::retain_in_transaction(&tx, &changed.to_string(), REVISION_LIMITS).await,
        Err(SubmissionStoreError::Conflict)
    ));
    tx.rollback().await.unwrap();
    assert!(
        client
            .execute(
                "DELETE FROM reborn_component_validator_selections WHERE submission_id=$1",
                &[&subject_id]
            )
            .await
            .is_err()
    );
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_component_validator_selections WHERE submission_id=$1",
                &[&subject_id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );

    // Even checksummed preparation metadata cannot replace the compiler's
    // actual step order. It remains unapproved and must fail restoration, never
    // become a feed or be silently replaced by a latest validator.
    let bad_subject_id = Uuid::new_v4();
    PgReviewSubmissionStore::new(rig.pool.clone())
        .submit(
            &ReviewSubmissionDraft::new(
                bad_subject_id,
                "preparation-fixture",
                brassclaw_skills::validator_components::structural_presence_draft(Uuid::new_v4())
                    .unwrap(),
                None,
                vec![],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let bad_subject = PgReviewSubmissionStore::read_validator_subject_in_transaction(
        &tx,
        bad_subject_id,
        REVISION_LIMITS,
    )
    .await
    .unwrap()
    .unwrap();
    let mut bad: Value = serde_json::from_str(
        &RetainedValidatorCoverage::selection_bytes(&bad_subject, &inspected, REVISION_LIMITS)
            .unwrap(),
    )
    .unwrap();
    let mut inner: Value =
        serde_json::from_str(bad["validator_selection_bytes"].as_str().unwrap()).unwrap();
    inner["step_order"].as_array_mut().unwrap().swap(0, 1);
    let inner = inner.to_string();
    {
        use sha2::{Digest, Sha256};
        bad["validator_selection_checksum"] =
            json!(format!("{:x}", Sha256::digest(inner.as_bytes())));
    }
    bad["validator_selection_bytes"] = json!(inner);
    selection::retain_in_transaction(&tx, &bad.to_string(), REVISION_LIMITS)
        .await
        .unwrap();
    let bad_saved =
        selection::read_preparation_in_transaction(&tx, bad_subject_id, REVISION_LIMITS)
            .await
            .unwrap()
            .unwrap();
    assert!(RetainedValidatorCoverage::restore_preparation(&bad_saved).is_err());
    tx.commit().await.unwrap();
    assert!(matches!(
        crate::pg_review_submission::get(&rig.pool, &bad_subject_id.to_string()).await,
        Err(brassclaw_product_workflow::RecipeStoreError::Invalid(_))
    ));
}

#[tokio::test]
async fn validator_aggregation_requires_exact_coverage_and_consistent_results_in_monty() {
    let draft = aggregate_draft(Uuid::new_v4()).unwrap();
    let collector = collect_draft(Uuid::new_v4(), &["identity".into(), "behavior".into()]).unwrap();
    let collection_inputs =
        InputContract::from_value(&collector.document()["input_contract"], REVISION_LIMITS)
            .unwrap();
    let collection_output =
        ValueContract::result_from_value(&collector.document()["result_contract"], REVISION_LIMITS)
            .unwrap();
    let inputs =
        InputContract::from_value(&draft.document()["input_contract"], REVISION_LIMITS).unwrap();
    let output =
        ValueContract::result_from_value(&draft.document()["result_contract"], REVISION_LIMITS)
            .unwrap();
    let preload = brassclaw_engine::executor::retained_preload::PreloadDeclaration::parse(
        draft.document(),
        draft.dependencies(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        preload.default_export.as_deref(),
        Some("aggregate_validator_checks")
    );
    let limits =
        UtilityLimits::from_execution(brassclaw_host_api::MontyExecutionLimits::default()).unwrap();
    let worker = installed_worker().unwrap();
    // Exercise actual retained IBS and source qualification as well as pure
    // execution. Version one here is a test draft, never an approval record.
    let recipe_id = Uuid::new_v4();
    let recipe = brassclaw_skills::component_revision::ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1","uuid":recipe_id,"class_code":21,
            "document":{
                "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":[],"variable_patterns":[]}],
                "step_descriptions":[{"desc_idx":0,"label":"Collect and aggregate observed checks","yaml_source":"","steps":[
                    {"stepnumber":1,"knowledge":"orchestrator","goal":"Preserve complete rule results","content":"","type":"component","include":[collector.uuid()]},
                    {"stepnumber":2,"knowledge":"orchestrator","goal":"Require full rule coverage","content":"","type":"component","include":[draft.uuid()]}
                ]}],
                "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":collector.document()["input_contract"],"steps":{
                    "0:1":{"identity":{"kind":"task_input","reference":"{{vars.identity}}"},"behavior":{"kind":"task_input","reference":"{{vars.behavior}}"}},
                    "0:2":{
                    "required_rule_ids":{"kind":"constant","value":["identity","behavior"]},
                    "checks":{"kind":"result","step_id":"0:1","path":[]}
                }}}},
                "invocations":{"selected":{"0:1":"collect_validator_checks","0:2":"aggregate_validator_checks"}}
            },"dependencies":[collector.uuid(),draft.uuid()],"association":null
        }).to_string(),
    ).unwrap();
    let snapshot = brassclaw_skills::component_revision::RetainedComponentSnapshot::new(
        &[recipe_id],
        vec![
            recipe.at_version(1).unwrap(),
            collector.clone().at_version(1).unwrap(),
            draft.clone().at_version(1).unwrap(),
        ],
    )
    .unwrap();
    let instruction = brassclaw_engine::memory::retained_instruction::compile_retained_recipe(
        Arc::new(snapshot),
        recipe_id,
        "selected",
        brassclaw_engine::memory::retained_instruction::WorkflowClass::Deterministic,
    )
    .unwrap();
    let program =
        brassclaw_engine::memory::retained_inputs::prepare_retained_unbound_program(instruction)
            .unwrap();
    let inspected =
        brassclaw_engine::executor::retained_source::InspectedRetainedProgram::inspect_with_limits(
            brassclaw_engine::executor::retained_recipe::RetainedProgram::Unbound(Arc::new(
                program,
            )),
            &worker,
            limits,
        )
        .await
        .unwrap();
    assert_eq!(
        inspected.invocation("0:1"),
        Some("result = collect_validator_checks(inputs=inputs)")
    );
    assert_eq!(
        inspected.invocation("0:2"),
        Some("result = aggregate_validator_checks(inputs=inputs)")
    );
    assert_eq!(inspected.preload_order().len(), 2);
    assert!(inspected.preload_order().contains(&draft.uuid()));
    assert!(inspected.preload_order().contains(&collector.uuid()));
    assert!(
        inspected
            .program()
            .inputs()
            .bind_task_inputs(&json!({"checks":[],"required_rule_ids":[]}))
            .is_err(),
        "a candidate cannot override the pinned required-rule list"
    );
    let UtilityOutput::Inspected { structure } = utility::execute(
        &worker,
        UtilityRequest::InspectSource {
            source: AGGREGATE_SOURCE.into(),
            bounds: limits.bounds,
        },
        limits.process,
    )
    .await
    .unwrap() else {
        panic!("source inspection required")
    };
    assert!(structure.imports.is_empty());
    assert!(structure.direct_host_calls.is_empty());
    assert_eq!(structure.host_value_references, 0);
    assert_eq!(structure.preload.unwrap().functions.len(), 1);
    let good = json!({"required_rule_ids":["identity","behavior"],"checks":[
        {"rule_id":"behavior","ok":true,"errors":[]},
        {"rule_id":"identity","ok":true,"errors":[]}
    ]});
    let mut rejected = good.clone();
    rejected["checks"][0] = json!({"rule_id":"behavior","ok":false,"errors":["end-exclusive"]});
    let evaluate = |value: Value| UtilityRequest::Evaluate {
        source: format!("{AGGREGATE_SOURCE}\naggregate_validator_checks(inputs)\n"),
        inputs: BTreeMap::from([("inputs".into(), value)]),
        bounds: limits.bounds,
        max_compute_time: Duration::from_secs(5),
    };
    for (value, expected) in [
        (good.clone(), json!({"pass":true,"errors":[]})),
        (
            rejected.clone(),
            json!({"pass":false,"errors":[{"rule_id":"behavior","errors":["end-exclusive"]}]}),
        ),
        (good.clone(), json!({"pass":true,"errors":[]})),
    ] {
        let by_rule = json!({"behavior":value["checks"][0],"identity":value["checks"][1]});
        inspected
            .program()
            .inputs()
            .bind_task_inputs(&by_rule)
            .unwrap();
        collection_inputs.bind(&by_rule).unwrap();
        let UtilityOutput::Evaluated {
            value: collected,
            stdout,
        } = utility::execute(
            &worker,
            UtilityRequest::Evaluate {
                source: format!("{COLLECT_SOURCE}\ncollect_validator_checks(inputs)\n"),
                inputs: BTreeMap::from([("inputs".into(), by_rule)]),
                bounds: limits.bounds,
                max_compute_time: Duration::from_secs(5),
            },
            limits.process,
        )
        .await
        .unwrap()
        else {
            panic!("actual collection required")
        };
        assert!(stdout.is_empty());
        collection_output.validate(&collected).unwrap();
        assert_eq!(
            collected, value["checks"],
            "preserve every complete result and diagnostic"
        );
        let value = json!({"required_rule_ids":value["required_rule_ids"],"checks":collected});
        inputs.bind(&value).unwrap();
        let UtilityOutput::Evaluated {
            value: actual,
            stdout,
        } = utility::execute(&worker, evaluate(value), limits.process)
            .await
            .unwrap()
        else {
            panic!("actual evaluation required")
        };
        assert!(stdout.is_empty());
        output.validate(&actual).unwrap();
        assert_eq!(actual, expected);
    }
    let swapped = json!({"identity":good["checks"][0],"behavior":good["checks"][1]});
    collection_inputs.bind(&swapped).unwrap();
    assert!(
        utility::execute(
            &worker,
            UtilityRequest::Evaluate {
                source: format!("{COLLECT_SOURCE}\ncollect_validator_checks(inputs)\n"),
                inputs: BTreeMap::from([("inputs".into(), swapped)]),
                bounds: limits.bounds,
                max_compute_time: Duration::from_secs(5),
            },
            limits.process
        )
        .await
        .is_err(),
        "swapped labels cannot fabricate complete rule coverage"
    );
    let invalid = [
        json!({"required_rule_ids":[],"checks":[]}),
        json!({"required_rule_ids":["identity","identity"],"checks":good["checks"]}),
        json!({"required_rule_ids":["identity","behavior"],"checks":[good["checks"][0]]}),
        json!({"required_rule_ids":["identity","behavior"],"checks":[good["checks"][0],good["checks"][0]]}),
        json!({"required_rule_ids":["identity"],"checks":[{"rule_id":"unknown","ok":true,"errors":[]}]}),
        json!({"required_rule_ids":["identity"],"checks":[{"rule_id":"identity","ok":false,"errors":[]}]}),
        json!({"required_rule_ids":["identity"],"checks":[{"rule_id":"identity","ok":true,"errors":["contradiction"]}]}),
        json!({"required_rule_ids":["identity"],"checks":[{"rule_id":"identity","ok":1,"errors":[]}]}),
        json!({"required_rule_ids":["identity"],"checks":[{"rule_id":"identity","ok":false,"errors":[""]}]}),
        json!({"required_rule_ids":["identity"],"checks":[{"rule_id":"identity","ok":true,"errors":[],"approved":true}]}),
    ];
    for value in invalid {
        // Test the Python boundary even for malformed data rejected by contracts.
        assert!(
            utility::execute(&worker, evaluate(value), limits.process)
                .await
                .is_err()
        );
    }
    assert!(
        inputs
            .bind(&json!({"required_rule_ids":null,"checks":[]}))
            .is_err()
    );
    assert!(
        inputs
            .bind(&json!({"required_rule_ids":["identity"],"checks":[],"passed":true}))
            .is_err()
    );
    assert!(output.validate(&json!({"pass":1,"errors":[]})).is_err());
}

/// Development qualification of R03's pure projection using the actual worker.
/// This is not an activated Recipe or the protected root's response handler.
pub(crate) async fn assert_model_answer_projection(response: Value, expected: &str) {
    use brassclaw_skills::model_completion_components::{
        MODEL_ANSWER_TEXT_SOURCE, model_answer_text_draft,
    };
    let draft = model_answer_text_draft(Uuid::new_v4()).unwrap();
    let checksum = draft.checksum();
    let inputs =
        InputContract::from_value(&draft.document()["input_contract"], REVISION_LIMITS).unwrap();
    let output =
        ValueContract::result_from_value(&draft.document()["result_contract"], REVISION_LIMITS)
            .unwrap();
    let value = json!({"response":response});
    inputs.bind(&value).unwrap();
    let limits =
        UtilityLimits::from_execution(brassclaw_host_api::MontyExecutionLimits::default()).unwrap();
    let UtilityOutput::Evaluated {
        value: actual,
        stdout,
    } = utility::execute(
        &installed_worker().unwrap(),
        UtilityRequest::Evaluate {
            source: format!("{MODEL_ANSWER_TEXT_SOURCE}\nextract_model_answer(inputs)\n"),
            inputs: BTreeMap::from([("inputs".into(), value)]),
            bounds: limits.bounds,
            max_compute_time: Duration::from_secs(5),
        },
        limits.process,
    )
    .await
    .unwrap()
    else {
        panic!("actual typed Monty projection required")
    };
    assert!(stdout.is_empty());
    output.validate(&actual).unwrap();
    assert_eq!(actual, json!(expected));
    assert_eq!(draft.checksum(), checksum);
}

#[tokio::test]
async fn model_answer_projection_checks_actual_wire_profile_without_erasing_unknown_usage() {
    use brassclaw_skills::model_completion_components::{
        assistant_model_response_schema, model_answer_text_draft,
    };
    use brassclaw_turns::run_profile::{
        AssistantReply, LoopModelResponse, LoopModelUsage, ModelProfileId, ModelStreamChunk,
        ParentLoopOutput,
    };
    let text = "' ü\n{{vars.answer}} {\"ok\":false}\nraise RuntimeError('data')";
    let mut response = LoopModelResponse {
        chunks: vec![ModelStreamChunk {
            safe_text_delta: text.into(),
        }],
        safe_reasoning_deltas: vec![],
        output: ParentLoopOutput::AssistantReply(AssistantReply {
            content: text.into(),
        }),
        effective_model_profile_id: ModelProfileId::new("selected_model").unwrap(),
        usage: None,
    };
    let wire = serde_json::to_value(&response).unwrap();
    assert!(!wire.as_object().unwrap().contains_key("usage"));
    assert_model_answer_projection(wire.clone(), text).await;
    response.output = ParentLoopOutput::AssistantReply(AssistantReply { content: "".into() });
    response.safe_reasoning_deltas = vec!["retained observation".into()];
    response.usage = Some(LoopModelUsage {
        input_tokens: 17,
        output_tokens: 3,
        cache_read_input_tokens: u32::MAX,
        cache_creation_input_tokens: 0,
    });
    assert_model_answer_projection(serde_json::to_value(&response).unwrap(), "").await;
    let profile =
        ValueContract::result_from_value(&assistant_model_response_schema(), REVISION_LIMITS)
            .unwrap();
    let draft = model_answer_text_draft(Uuid::new_v4()).unwrap();
    let input =
        InputContract::from_value(&draft.document()["input_contract"], REVISION_LIMITS).unwrap();
    profile
        .require_compatible_input(&input, "response")
        .unwrap();
    let original = wire.clone();
    for path in [
        "/chunks",
        "/chunks/0/safe_text_delta",
        "/output",
        "/output/assistant_reply",
        "/output/assistant_reply/content",
        "/effective_model_profile_id",
    ] {
        let mut invalid = wire.clone();
        *invalid.pointer_mut(path).unwrap() = Value::Null;
        assert!(profile.validate(&invalid).is_err(), "{path}");
    }
    for key in ["chunks", "output", "effective_model_profile_id"] {
        let mut invalid = wire.clone();
        invalid.as_object_mut().unwrap().remove(key);
        assert!(profile.validate(&invalid).is_err(), "{key}");
    }
    for (key, value) in [
        ("usage", Value::Null),
        ("safe_reasoning_deltas", Value::Null),
        ("unknown", json!(true)),
    ] {
        let mut invalid = wire.clone();
        invalid[key] = value;
        assert!(profile.validate(&invalid).is_err(), "{key}");
    }
    let known = serde_json::to_value(&response).unwrap();
    for value in [
        json!(-1),
        json!(4294967296_u64),
        json!(true),
        json!(1.5),
        Value::Null,
    ] {
        let mut invalid = known.clone();
        invalid["usage"]["input_tokens"] = value;
        assert!(profile.validate(&invalid).is_err());
    }
    response.output = ParentLoopOutput::CapabilityCalls(vec![]);
    assert!(
        profile
            .validate(&serde_json::to_value(&response).unwrap())
            .is_err()
    );
    profile.validate(&wire).unwrap();
    assert_eq!(wire, original);
    assert!(model_answer_text_draft(Uuid::nil()).is_err());
}
