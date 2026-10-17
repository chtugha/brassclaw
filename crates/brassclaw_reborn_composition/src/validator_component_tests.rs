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
        "document":{"content":"def inspect_identity(inputs):\n    return {'rule_id': 'identity', 'ok': inputs['subject'] != '', 'errors': [] if inputs['subject'] != '' else ['subject missing']}",
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
        process::{ProcessBoundary, WorkerCommand},
        transport_actor::{ActorLimits, TransportOwner},
    };
    let source = "import asyncio\nasync def observe():\n    task = await host.await_next_task(0)\n    host.enter_task(task['task_token'])\n    await host.validation_hold(task['task_token'])\nasyncio.run(observe())\n";
    let mut boot = observation_runtime::boot(source);
    boot.bounds.workers = 1;
    boot.aliases.insert("validation_hold".into());
    let (mut owner, ready) = TransportOwner::start(
        observation_runtime::worker(),
        boot,
        observation_runtime::limits(),
        ActorLimits {
            max_unclaimed: 8,
            max_reserved_frame_bytes: 4 * 1024 * 1024,
            max_control_unclaimed: 8,
            max_control_reserved_frame_bytes: 4 * 1024 * 1024,
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
