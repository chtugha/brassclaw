//! Actual PostgreSQL -> retained IBS -> typed preparation. These unapproved
//! draft workflows exercise validation, not production activation or dispatch.
#![cfg(feature = "skills-db")]

use std::{sync::Arc, time::Duration};

use brassclaw_engine::executor::retained_recipe::{
    RetainedExecutionError, RetainedProgram, RetainedRecipeExecution,
};
use brassclaw_engine::executor::retained_source::InspectedRetainedProgram;
use brassclaw_engine::memory::{
    retained_inputs::{
        RetainedInputError, RetainedUnboundProgram, prepare_retained_unbound_program,
    },
    retained_instruction::{WorkflowClass, compile_retained_recipe},
};
use brassclaw_monty_host::{
    VmBounds,
    global::GlobalBounds,
    heap::HeapSettings,
    process::{
        PortAnswer, ProcessBoundary, ProcessLimits, ProcessSnapshot, RootBoot, TaskSettings,
        WorkerCommand, installed_worker,
    },
    transport_actor::{ActorLimits, TransportClient, TransportOwner},
};
use brassclaw_skills::{
    association_contract::ComponentRevisionRef, component_revision::ComponentRevisionDraft,
    revision_store::PgComponentRevisionStore,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[path = "../../brassclaw_reborn/tests/common/native_pg.rs"]
mod native_pg;

fn draft(id: Uuid, class: i32, document: Value, dependencies: &[Uuid]) -> ComponentRevisionDraft {
    ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1","uuid":id,"class_code":class,
        "document":document,"dependencies":dependencies,"association":null})
        .to_string(),
    )
    .unwrap()
}
fn code(text: &str, input: &str) -> Value {
    json!({"content":text,"includes":[],"dependency_registry":null,"input_contract":{
        (input):{"type":"string","required":true,"checks":[]}},"result_contract":{
        "type":"object","allow_extra_fields":false,"fields":{
            "text":{"type":"string","required":true}}}})
}
fn recipe(first: Uuid, second: Uuid) -> Value {
    json!({"variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":["show %"],
        "variable_patterns":[{"name":"value","pattern":null,"description":null}]}],
        "step_descriptions":[{"desc_idx":0,"label":"typed flow","yaml_source":"","steps":[
            {"stepnumber":1,"knowledge":"orchestrator","goal":"Prepare typed text","content":"","type":"component","include":[first]},
            {"stepnumber":2,"knowledge":"orchestrator","goal":"Consume the prior result","content":"","type":"component","include":[second]}]}],
        "input_layouts":{"selected":{"format":"recipe-input-layout/1",
            "task_inputs":{"value":{"type":"string","required":true,"checks":[]}},
            "steps":{"0:1":{"text":{"kind":"task_input","reference":"{{vars.value}}"}},
                "0:2":{"previous":{"kind":"result","step_id":"0:1","path":["text"]}}}}}})
}
async fn prepare(
    store: &PgComponentRevisionStore,
    root: Uuid,
    refs: &[ComponentRevisionRef],
) -> Result<RetainedUnboundProgram, brassclaw_engine::memory::retained_inputs::RetainedInputError> {
    let instruction = compile_retained_recipe(
        Arc::new(store.read_exact(&[root], refs).await.unwrap()),
        root,
        "selected",
        WorkflowClass::Deterministic,
    )
    .unwrap();
    prepare_retained_unbound_program(instruction)
}

async fn exchange(process: &TransportClient, command: WorkerCommand) -> ProcessSnapshot {
    process
        .try_submit(command)
        .unwrap()
        .wait()
        .await
        .unwrap()
        .outcome
        .unwrap()
}

async fn root_progress(process: &TransportClient, mut state: ProcessSnapshot) -> ProcessSnapshot {
    while let Some(ProcessBoundary::ControlYield { key }) = state.boundary {
        state = exchange(process, WorkerCommand::ResumeControl { key }).await;
    }
    state
}

/// Draft behavioral validation through the actual global Recipe helper and one
/// retained child context. The special entry does no matching, Tool dispatch,
/// reply posting or activation. Host transport returns actual compiler/VM data.
async fn execute_draft_flow(
    program: Arc<RetainedUnboundProgram>,
    inputs: Value,
    expect_invalid_result: bool,
) -> Result<Value, Value> {
    let definitions = include_str!("../orchestrator/global_mode.py")
        .strip_suffix("asyncio.run(_global_main())\n")
        .unwrap();
    let source = format!(
        "{definitions}\nasync def validate_draft():\n    task = await host.await_next_task(0)\n    host.enter_task(task['task_token'])\n    value = await _execute_recipe(task['task_token'], 'retained-draft', '0:1-0:E', {{'user_input': task['user_input']}})\n    await host.validation_result(task['task_token'], value)\nasyncio.run(validate_draft())\n"
    );
    let executable = installed_worker().unwrap();
    let (mut owner, ready) = TransportOwner::start(
        &executable,
        RootBoot {
            adapter_reserve_bytes: brassclaw_host_api::DEFAULT_MONTY_ADAPTER_RESERVE_BYTES as usize,
            checksum: Sha256::digest(source.as_bytes()).into(),
            source,
            aliases: [
                "await_next_task",
                "enter_task",
                "compose_orchestrator",
                "run_program",
                "validation_result",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            heap_settings: Some(HeapSettings {
                revision: 1,
                max_vm_bytes: 16 * 1024 * 1024,
            }),
            bounds: GlobalBounds {
                workers: 1,
                max_pending_calls: 8,
                values: VmBounds {
                    max_source_bytes: 65_536,
                    max_compiled_source_bytes: 131_072,
                    max_feeds: 32,
                    max_stdout_bytes: 1024,
                    execution_slice: Duration::from_millis(5),
                    max_value_depth: 48,
                    max_value_nodes: 4096,
                    max_value_bytes: 65_536,
                },
            },
            startup_timeout: Duration::from_secs(10),
            task_settings: TaskSettings {
                revision: 1,
                max_compute_time: Duration::from_secs(600),
                token_budgets_enabled: false,
            },
            max_recipe_contexts: 4,
        },
        ProcessLimits {
            hard_memory_bytes: 64 * 1024 * 1024,
            max_frame_bytes: 256 * 1024,
            response_timeout: Duration::from_secs(5),
        },
        ActorLimits {
            max_unclaimed: 8,
            max_reserved_frame_bytes: 4 * 1024 * 1024,
            max_control_unclaimed: 8,
            max_control_reserved_frame_bytes: 4 * 1024 * 1024,
        },
    )
    .await
    .unwrap();
    let process = owner.client();
    assert_eq!(ready.work_waits.len(), 1);
    let admitted = exchange(&process, WorkerCommand::Admit { key: ready.work_waits[0].1,
        task: json!({"conversation_id":"opaque-draft-conversation", "message_id":"draft-message", "turn_id":"draft-turn", "run_id":"draft-run", "user_input":inputs["value"], "history":[]}) }).await;
    let task = admitted.admitted_task.unwrap();
    let admitted = root_progress(&process, admitted).await;
    let Some(ProcessBoundary::HostCall {
        key,
        name,
        args,
        kwargs,
    }) = admitted.boundary
    else {
        panic!("actual composition request required")
    };
    assert_eq!(name, "compose_orchestrator");
    assert!(kwargs.is_empty());
    assert_eq!(args.len(), 4);
    assert_eq!(args[1], "retained-draft");
    assert_eq!(
        args[2],
        program
            .inputs()
            .instruction()
            .variant()
            .step_link
            .as_deref()
            .unwrap()
    );
    let token = args[0].clone();
    let reference = Uuid::new_v4().to_string();
    let step_ids: Vec<_> = program
        .program()
        .steplist
        .iter()
        .map(|step| json!({"step_id":step.step_id}))
        .collect();
    exchange(&process, WorkerCommand::Defer { key }).await;
    let resolved = exchange(
        &process,
        WorkerCommand::Resolve {
            key,
            answer: PortAnswer::Return {
                value: json!({"ok":true, "program_ref":reference, "steps":step_ids,
            "inputs":inputs, "flow":program.inputs().monty_flow().unwrap()}),
            },
        },
    )
    .await;
    let mut root = root_progress(&process, resolved).await;
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect(
            RetainedProgram::Unbound(program.clone()),
            &installed_worker().unwrap(),
        )
        .await
        .unwrap(),
    );
    let mut execution = RetainedRecipeExecution::new(task, inspected).unwrap();
    let mut observed = Vec::new();
    for step in &program.program().steplist {
        let Some(ProcessBoundary::HostCall {
            key,
            name,
            args,
            kwargs,
        }) = root.boundary
        else {
            panic!("actual step request required")
        };
        assert_eq!(name, "run_program");
        assert!(kwargs.is_empty());
        assert_eq!(args.len(), 4);
        assert_eq!(args[0], token);
        assert_eq!(args[1], reference);
        assert_eq!(args[2], step.step_id);
        exchange(&process, WorkerCommand::Defer { key }).await;
        observed.push(step.step_id.clone());
        let result = execution
            .run_step(&process, &step.step_id, &args[3]["inputs"], None)
            .await;
        if expect_invalid_result {
            assert!(matches!(result, Err(RetainedExecutionError::Inputs(_))));
            let Some(brassclaw_monty_host::process::RecipeEvent::Progress {
                boundary: brassclaw_monty_host::process::RecipeBoundary::Complete { value },
                ..
            }) = execution.latest_snapshot().unwrap().recipe.as_ref()
            else {
                panic!("actual rejected result must remain observable")
            };
            let actual = value.clone();
            // Both replay and another step are fenced after an actual output violation.
            assert!(
                execution
                    .run_step(&process, &step.step_id, &args[3]["inputs"], None)
                    .await
                    .is_err()
            );
            assert!(
                execution
                    .run_step(&process, "0:2", &json!({"previous":"other"}), None)
                    .await
                    .is_err()
            );
            assert!(execution.host_answers().is_empty());
            owner.request_termination();
            let exit = owner.join().await.unwrap();
            assert!(exit.exit_status.is_some());
            assert!(exit.containment_error.is_none());
            assert!(exit.reap_error.is_none());
            return Err(actual);
        }
        let value = result.unwrap();
        // A repeated request cannot replay a completed component or its effects.
        assert!(
            execution
                .run_step(&process, &step.step_id, &args[3]["inputs"], None)
                .await
                .is_err()
        );
        assert!(execution.host_answers().is_empty());
        assert!(execution.latest_snapshot().unwrap().recipe.is_some());
        let resolved = exchange(
            &process,
            WorkerCommand::Resolve {
                key,
                answer: PortAnswer::Return {
                    value: json!({"ok":true,"return_value":value}),
                },
            },
        )
        .await;
        root = root_progress(&process, resolved).await;
    }
    assert_eq!(observed, ["0:1", "0:2"]);
    let Some(ProcessBoundary::HostCall {
        name, args, kwargs, ..
    }) = root.boundary
    else {
        panic!("actual validation result required")
    };
    assert_eq!(name, "validation_result");
    assert!(kwargs.is_empty());
    assert_eq!(args.len(), 2);
    assert_eq!(args[0], token);
    let value = args.into_iter().nth(1).unwrap();
    // The validation port is intentionally not acknowledged as a product task
    // finish or an external effect. No Tool ran; containment is an actual reap.
    owner.request_termination();
    let exit = owner.join().await.unwrap();
    assert!(exit.exit_status.is_some());
    assert!(exit.containment_error.is_none());
    assert!(exit.reap_error.is_none());
    Ok(value)
}

#[tokio::test]
async fn retained_contracts_and_bindings_survive_replacement_without_source_substitution() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let (root, first, second) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let first_body = "result = {'text': inputs['text']}";
    let second_body = "result = {'text': inputs['previous']}";
    let first_ref = store
        .stage(&draft(first, 22, code(first_body, "text"), &[]), 0)
        .await
        .unwrap();
    let second_ref = store
        .stage(&draft(second, 22, code(second_body, "previous"), &[]), 0)
        .await
        .unwrap();
    let root_ref = store
        .stage(&draft(root, 21, recipe(first, second), &[first, second]), 0)
        .await
        .unwrap();
    let refs = [root_ref, first_ref, second_ref];
    let assembled = Arc::new(prepare(&store, root, &refs).await.unwrap());
    assert_eq!(assembled.program().steplist.len(), 2);
    assert_eq!(assembled.program().steplist[0].executable_code, first_body);
    assert_eq!(assembled.program().steplist[1].executable_code, second_body);
    assert!(assembled.program().variables.is_empty());
    assert!(assembled.program().assembled_program.is_empty());
    assert!(assembled.program().rust_directives.is_empty());
    let selected = assembled.inputs();
    let supplied = json!({"value":"'quoted'\n\\ Ü {{vars.value}} host.forbidden()"});
    let bound = selected.bind_task_inputs(&supplied).unwrap();
    assert_eq!(bound, supplied);
    let captured = selected
        .bind_variant_example(
            "show %",
            &format!("show {}", supplied["value"].as_str().unwrap()),
            &json!({}),
        )
        .unwrap();
    assert_eq!(captured, supplied);
    assert!(
        selected
            .bind_variant_example("other %", "other text", &json!({}))
            .is_err()
    );
    assert!(
        selected
            .bind_variant_example("show %", "ignored show text", &json!({}))
            .is_err()
    );
    assert!(
        selected
            .bind_variant_example(
                "show %",
                "show text ignored",
                &json!({"value":"replacement"})
            )
            .is_err()
    );
    let layout = serde_json::to_value(selected.layout().steps()).unwrap();
    assert_eq!(
        layout["0:1"]["text"],
        json!({"kind":"input","name":"value"})
    );
    assert_eq!(
        layout["0:2"]["previous"],
        json!({"kind":"result","step_id":"0:1","path":["text"]})
    );
    let concrete = json!({"text":bound["value"]});
    assert_eq!(
        selected.bind_step_inputs("0:1", &concrete).unwrap(),
        concrete
    );
    selected.validate_step_result("0:1", &concrete).unwrap();
    let second_input = json!({"previous":concrete["text"]});
    assert_eq!(
        selected.bind_step_inputs("0:2", &second_input).unwrap(),
        second_input
    );
    // The actual global helper owns step order and the result handoff. Its
    // admitted envelope has different names from the prepared Recipe inputs.
    let value = execute_draft_flow(assembled.clone(), bound, false)
        .await
        .unwrap();
    assert_eq!(value, concrete);
    selected.validate_step_result("0:2", &value).unwrap();
    for invalid in [
        json!({}),
        json!({"text":null}),
        json!({"text":false}),
        json!({"text":"ok","extra":"undeclared"}),
    ] {
        assert!(selected.bind_step_inputs("0:1", &invalid).is_err());
        assert!(selected.validate_step_result("0:1", &invalid).is_err());
    }
    assert!(selected.bind_step_inputs("absent", &json!({})).is_err());
    assert!(selected.bind_task_inputs(&json!({"value":5})).is_err());
    let mut incompatible = code("result = {'text': 1}", "text");
    incompatible["result_contract"]["fields"]["text"]["type"] = json!("integer");
    let replacement = store
        .stage(&draft(first, 22, incompatible, &[]), 1)
        .await
        .unwrap();
    assert!(
        prepare(&store, root, &[root_ref, replacement, second_ref])
            .await
            .is_err()
    );
    let retained = prepare(&store, root, &refs).await.unwrap();
    let resumed = retained.inputs();
    assert_eq!(resumed.components()["0:1"], first_ref);
    assert_eq!(resumed.bind_task_inputs(&supplied).unwrap(), supplied);
    assert_eq!(
        resumed.instruction().snapshot().revisions()[&first]
            .draft()
            .document()["content"],
        first_body
    );
}

#[tokio::test]
async fn malformed_or_missing_retained_binding_metadata_fails_before_execution() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let (root, first, second) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let first_ref = store
        .stage(
            &draft(
                first,
                22,
                code("result = {'text': inputs['text']}", "text"),
                &[],
            ),
            0,
        )
        .await
        .unwrap();
    let second_ref = store
        .stage(
            &draft(
                second,
                22,
                code("result = {'text': inputs['previous']}", "previous"),
                &[],
            ),
            0,
        )
        .await
        .unwrap();
    let valid = recipe(first, second);
    let initial = store
        .stage(&draft(root, 21, valid.clone(), &[first, second]), 0)
        .await
        .unwrap();
    prepare(&store, root, &[initial, first_ref, second_ref])
        .await
        .unwrap();
    let mut cases = Vec::new();
    let mut bad = valid.clone();
    bad.as_object_mut().unwrap().remove("input_layouts");
    cases.push(bad);
    let mut bad = valid.clone();
    bad["input_layouts"]["selected"]["steps"]["0:1"]["text"]["reference"] =
        json!("prefix {{vars.value}}");
    cases.push(bad);
    let mut bad = valid.clone();
    bad["input_layouts"]["selected"]["steps"]["0:1"]["text"] =
        json!({"kind":"result","step_id":"0:2","path":["text"]});
    cases.push(bad);
    let mut bad = valid.clone();
    bad["input_layouts"]["selected"]["steps"]["0:2"]["previous"]["path"] = json!(["missing"]);
    cases.push(bad);
    let mut bad = valid.clone();
    bad["input_layouts"]["selected"]["steps"]["0:1"]["text"]["execute"] = json!(true);
    cases.push(bad);
    let mut bad = valid.clone();
    bad["input_layouts"]["selected"]["steps"]["0:3"] = json!({});
    cases.push(bad);
    let mut bad = valid;
    bad["input_layouts"]["selected"]["steps"]["0:2"] = json!({});
    cases.push(bad);
    let expected = [
        "retained Recipe input layouts required",
        "invalid whole-value input reference",
        "result producer is not a completed predecessor",
        "incompatible input contract",
        "missing or unsupported metadata field",
        "contracts and bindings must cover exactly the selected executable steps",
        "every declared local input needs an explicit binding",
    ];
    for ((previous, document), reason) in (1..).zip(cases).zip(expected) {
        let root_ref = store
            .stage(&draft(root, 21, document, &[first, second]), previous)
            .await
            .unwrap();
        let error = match prepare(&store, root, &[root_ref, first_ref, second_ref]).await {
            Err(RetainedInputError::Invalid(reason)) => reason,
            Err(RetainedInputError::Binding(error)) => error.reason,
            _ => panic!("expected the specific invalid declaration to fail"),
        };
        assert_eq!(error, reason);
    }
    let missing_contract = store
        .stage(
            &draft(first, 22, json!({"content":"result = None"}), &[]),
            1,
        )
        .await
        .unwrap();
    let root_ref = store
        .stage(&draft(root, 21, recipe(first, second), &[first, second]), 8)
        .await
        .unwrap();
    assert!(
        prepare(&store, root, &[root_ref, missing_contract, second_ref])
            .await
            .is_err()
    );
    // Real stored leaf defaults are accepted, but malformed/nonempty include
    // metadata cannot be mistaken for a leaf and discarded during assembly.
    for (previous, (includes, reason)) in (2..).zip([
        (json!(null), "PythonCode includes must be an array"),
        (json!({}), "PythonCode includes must be an array"),
        (json!("[]"), "PythonCode includes must be an array"),
        (
            json!([Uuid::new_v4()]),
            "nested PythonCode requires explicit retained assembly",
        ),
    ]) {
        let mut document = code("result = {'text': inputs['text']}", "text");
        document["includes"] = includes;
        let replacement = store
            .stage(&draft(first, 22, document, &[]), previous)
            .await
            .unwrap();
        assert!(matches!(
            prepare(&store, root, &[root_ref, replacement, second_ref]).await,
            Err(RetainedInputError::Invalid(actual)) if actual == reason
        ));
    }
    // Exact original selection remains executable after rejected replacements.
    prepare(&store, root, &[root_ref, first_ref, second_ref])
        .await
        .unwrap();
}

#[tokio::test]
async fn retained_capture_requires_complete_refinement_and_never_demotes_to_raw_slots() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let (root, first, second) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let first_ref = store
        .stage(
            &draft(
                first,
                22,
                code("result = {'text': inputs['text']}", "text"),
                &[],
            ),
            0,
        )
        .await
        .unwrap();
    let second_ref = store
        .stage(
            &draft(
                second,
                22,
                code("result = {'text': inputs['previous']}", "previous"),
                &[],
            ),
            0,
        )
        .await
        .unwrap();
    let mut document = recipe(first, second);
    document["variants"][0]["intent_examples"] = json!(["show % in % directory"]);
    document["variants"][0]["variable_patterns"] = json!([
        {"name":"value","pattern":"^(?P<value>[A-Z]{3}-[0-9]{3})$","description":null},
        {"name":"place","pattern":null,"description":null}]);
    document["input_layouts"]["selected"]["task_inputs"]["place"] =
        json!({"type":"string","required":true,"checks":[]});
    let first_recipe = store
        .stage(&draft(root, 21, document.clone(), &[first, second]), 0)
        .await
        .unwrap();
    let assembled = prepare(&store, root, &[first_recipe, first_ref, second_ref])
        .await
        .unwrap();
    let selected = assembled.inputs();
    let template = "show % in % directory";
    let place = "'/quoted'\\\nÜ in other {{vars.value}}";
    assert_eq!(
        selected
            .bind_variant_example(
                template,
                &format!("show ABC-123 in {place} directory"),
                &json!({})
            )
            .unwrap(),
        json!({"value":"ABC-123","place":place})
    );
    assert_eq!(
        selected
            .bind_variant_example(template, "show ABC-123 in  directory", &json!({}))
            .unwrap(),
        json!({"value":"ABC-123","place":""})
    );
    for input in [
        "show invalid in /tmp directory",
        "show ABC-123 /tmp directory",
        "show ABC-123 in /tmp",
        "prefix show ABC-123 in /tmp directory",
        "show ABC-123 in /tmp directory suffix",
    ] {
        assert!(
            selected
                .bind_variant_example(template, input, &json!({}))
                .is_err()
        );
    }
    assert!(
        selected
            .bind_variant_example(
                template,
                "show ABC-123 in /tmp directory",
                &json!({"place":"injected"})
            )
            .is_err()
    );
    // Invalid authoring is rejected during preparation, before user capture.
    let mut cases = Vec::new();
    let mut bad = document.clone();
    bad["variants"][0]["variable_patterns"][0]["pattern"] = json!("[");
    cases.push((bad, "invalid retained capture pattern"));
    let mut bad = document.clone();
    bad["variants"][0]["variable_patterns"][0]["pattern"] = json!("^(?P<other>.*)$");
    cases.push((bad, "capture group must name its declared input"));
    let mut bad = document.clone();
    bad["variants"][0]["variable_patterns"][1]["name"] = json!("value");
    cases.push((bad, "capture name is undeclared or duplicate"));
    for (previous, (bad, reason)) in (1..).zip(cases) {
        let reference = store
            .stage(&draft(root, 21, bad, &[first, second]), previous)
            .await
            .unwrap();
        assert!(
            matches!(prepare(&store, root, &[reference, first_ref, second_ref]).await, Err(RetainedInputError::Invalid(actual)) if actual == reason)
        );
    }
    // A partial regex match is not full validation; the raw suffix never flows
    // into a Tool or falls back to a positional slot.
    document["variants"][0]["variable_patterns"][0]["pattern"] = json!("[A-Z]{3}-[0-9]{3}");
    let partial = store
        .stage(&draft(root, 21, document.clone(), &[first, second]), 4)
        .await
        .unwrap();
    let prepared = prepare(&store, root, &[partial, first_ref, second_ref])
        .await
        .unwrap();
    assert!(matches!(
        prepared.inputs().bind_variant_example(
            template,
            "show ABC-123 bad in /tmp directory",
            &json!({})
        ),
        Err(RetainedInputError::Invalid(
            "capture refinement must validate the complete slot"
        ))
    ));
    // An optional named group that did not participate must not reuse raw data.
    document["variants"][0]["variable_patterns"][0]["pattern"] = json!("^(?P<value>ABC-123)?$");
    let optional = store
        .stage(&draft(root, 21, document, &[first, second]), 5)
        .await
        .unwrap();
    let prepared = prepare(&store, root, &[optional, first_ref, second_ref])
        .await
        .unwrap();
    assert!(matches!(
        prepared
            .inputs()
            .bind_variant_example(template, "show  in /tmp directory", &json!({})),
        Err(RetainedInputError::Invalid(
            "declared capture group did not participate"
        ))
    ));
    // The original selection still holds its original refinement and bytes.
    let resumed = prepare(&store, root, &[first_recipe, first_ref, second_ref])
        .await
        .unwrap();
    assert!(
        resumed
            .inputs()
            .bind_variant_example(template, "show invalid in /tmp directory", &json!({}))
            .is_err()
    );
}

#[tokio::test]
async fn actual_invalid_step_output_fences_the_retained_execution_without_replay() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let (root, first, second) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let first_ref = store
        .stage(
            &draft(first, 22, code("result = {'text': 7}", "text"), &[]),
            0,
        )
        .await
        .unwrap();
    let second_ref = store
        .stage(
            &draft(
                second,
                22,
                code("result = {'text': inputs['previous']}", "previous"),
                &[],
            ),
            0,
        )
        .await
        .unwrap();
    let root_ref = store
        .stage(&draft(root, 21, recipe(first, second), &[first, second]), 0)
        .await
        .unwrap();
    let prepared = Arc::new(
        prepare(&store, root, &[root_ref, first_ref, second_ref])
            .await
            .unwrap(),
    );
    let actual = execute_draft_flow(prepared, json!({"value":"data"}), true)
        .await
        .unwrap_err();
    assert_eq!(actual, json!({"text":7}));
}
