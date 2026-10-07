//! Actual immutable-draft -> contained inspection -> structural Q1 persistence.
//! Actual observed behavior and explicit human decisions are retained separately.
//! No catalogue activation or whole-workflow completion is invented.

use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;

use brassclaw_engine::executor::{
    retained_recipe::{
        RetainedProgram, RetainedRecipeExecution, RetainedStepFailure, RetainedToolInvocation,
        RetainedToolPort,
    },
    retained_source::InspectedRetainedProgram,
};
use brassclaw_host_api::{
    CapabilityId, CapabilitySet, ExecutionContext, ExtensionId, MountView, ResourceEstimate,
    RuntimeKind, TrustClass, UserId,
};
use brassclaw_host_runtime::{
    RetainedFirstPartyCapability, RuntimeCapabilityOutcome, RuntimeCapabilityRequest,
};
use brassclaw_monty_host::{
    process::{PortAnswer, ProcessBoundary, ProcessSnapshot, TaskHandle, WorkerCommand},
    transport_actor::{ActorLimits, TransportClient, TransportOwner},
};
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft, revision_store::PgComponentRevisionStore,
};
use brassclaw_turns::{GetRunStateRequest, TurnStateStore};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[path = "support/admission.rs"]
mod admission;
#[path = "../../../crates/brassclaw_reborn/tests/common/native_pg.rs"]
mod native_pg;
#[path = "../../../crates/brassclaw_reborn_composition/src/pg_monty_admission.rs"]
mod pg_monty_admission;
#[path = "../../../crates/brassclaw_reborn_composition/src/pg_retained_q1.rs"]
mod pg_retained_q1;
#[path = "../../../crates/brassclaw_reborn_composition/src/pg_workflow_review.rs"]
mod pg_workflow_review;
#[path = "support/retained_kernel.rs"]
mod retained_kernel;
#[path = "support/retained_program.rs"]
mod retained_program;
#[path = "support/runtime.rs"]
mod support;

use pg_retained_q1::{
    BehavioralExpectation, BehavioralReviewSet, ExpectedStepResult, StructuralReviewFailure,
    StructuralReviewSet, persist_behavioral_reviews, persist_structural_reviews,
};

fn worker() -> &'static std::path::Path {
    support::worker()
}

#[test]
fn observed_value_fingerprints_preserve_types_presence_and_answer_classification() {
    use brassclaw_engine::executor::retained_recipe::{port_answer_checksum, typed_value_checksum};
    assert_ne!(
        typed_value_checksum(&Value::Null).unwrap(),
        typed_value_checksum(&json!({})).unwrap()
    );
    assert_ne!(
        typed_value_checksum(&json!(1)).unwrap(),
        typed_value_checksum(&json!(1.0)).unwrap()
    );
    assert_ne!(
        typed_value_checksum(&json!("1")).unwrap(),
        typed_value_checksum(&json!(1)).unwrap()
    );
    let first: Value = serde_json::from_str("{\"a\":1,\"b\":null}").unwrap();
    let second: Value = serde_json::from_str("{\"b\":null,\"a\":1}").unwrap();
    assert_eq!(
        typed_value_checksum(&first).unwrap(),
        typed_value_checksum(&second).unwrap()
    );
    assert_ne!(
        typed_value_checksum(&first).unwrap(),
        typed_value_checksum(&json!({"a":1})).unwrap()
    );
    assert_ne!(
        port_answer_checksum(&PortAnswer::DomainError {
            reason_kind: "tool_policy_denied".into()
        })
        .unwrap(),
        port_answer_checksum(&PortAnswer::TerminalError {
            reason_kind: "tool_policy_denied".into()
        })
        .unwrap()
    );
    let mut deep = Value::Null;
    for _ in 0..66 {
        deep = json!([deep]);
    }
    assert!(
        typed_value_checksum(&deep).is_err(),
        "oversized depth fails before recursive fingerprinting"
    );
}

#[tokio::test]
async fn actual_structural_reviews_retain_exact_source_and_are_not_combination_approval() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let program = retained_program::program(&store, false).await;
    let binding = &program.bindings()["0:2"];
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect(RetainedProgram::Tools(program.clone()), worker())
            .await
            .unwrap(),
    );
    let workflow =
        Arc::new(pg_workflow_review::WorkflowReview::structural(inspected.clone()).unwrap());
    let workflow_value: Value = serde_json::from_str(workflow.evidence()).unwrap();
    assert_eq!(workflow_value["kind"], "q1");
    assert_eq!(
        workflow_value["report"]["scope"],
        "selected-variant-structure"
    );
    assert_eq!(
        workflow_value["report"]["sources"]
            .as_object()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(workflow_value["report"]["semantic_approval"], false);
    assert_eq!(workflow_value["report"]["task_completion"], false);
    pg_workflow_review::persist_workflow_review(&rig.pool, workflow.clone())
        .await
        .unwrap();
    pg_workflow_review::persist_workflow_review(&rig.pool, workflow.clone())
        .await
        .unwrap();
    let reviews = Arc::new(StructuralReviewSet::prepare(inspected).unwrap());
    assert_eq!(reviews.references().len(), 1);
    assert_eq!(reviews.inspected().source_checks().len(), 1);
    let skill = binding.skill().uuid;
    let id = reviews.references()[&skill];
    let bytes = reviews.evidence(skill).unwrap();
    let evidence: Value = serde_json::from_str(bytes).unwrap();
    assert_eq!(evidence["kind"], "q1");
    assert_eq!(evidence["validation_mode"], "authored");
    assert_eq!(evidence["succeeded"], true);
    assert_eq!(evidence["report"]["scope"], "structural-only");
    assert_eq!(evidence["report"]["semantic_approval"], false);
    assert_eq!(
        evidence["association_checksum"],
        format!(
            "{:x}",
            Sha256::digest(binding.association().exact_bytes().as_bytes())
        )
    );
    assert_eq!(
        evidence["components"].as_array().unwrap().len(),
        binding.combination().len()
    );
    assert_eq!(evidence["reviewed_evidence"], json!([]));
    assert_eq!(
        evidence["report"]["observations"]["source_checksum"],
        reviews.inspected().source_checks()[&binding.python().uuid]
            .observations()
            .source_checksum
    );
    persist_structural_reviews(&rig.pool, reviews.clone())
        .await
        .unwrap();
    persist_structural_reviews(&rig.pool, reviews.clone())
        .await
        .unwrap();
    // A real replacement does not change the exact original review target.
    let selected = &program.inputs().instruction().snapshot().revisions()[&binding.python().uuid];
    let mut document = selected.draft().document().clone();
    document["content"] =
        json!("result = host.json(operation='parse', data=inputs['data'])\n# replacement revision");
    let replacement = ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1", "uuid":binding.python().uuid,
        "class_code":22, "document":document, "dependencies":[], "association":null})
        .to_string(),
    )
    .unwrap();
    let newer = store
        .stage(&replacement, binding.python().version)
        .await
        .unwrap();
    assert_ne!(newer, binding.python());
    persist_structural_reviews(&rig.pool, reviews.clone())
        .await
        .unwrap();
    let client = rig.pool.get().await.unwrap();
    let actual = client.query_one(
        "SELECT evidence_bytes,checksum FROM reborn_component_review_evidence WHERE evidence_id=$1",
        &[&id],
    ).await.unwrap();
    assert_eq!(actual.get::<_, String>(0), bytes);
    assert_eq!(
        actual.get::<_, String>(1),
        format!("{:x}", Sha256::digest(bytes.as_bytes()))
    );
    let counts = client
        .query_one(
            "SELECT (SELECT count(*) FROM reborn_component_review_evidence),
                (SELECT count(*) FROM reborn_skill_association_approvals),
                (SELECT count(*) FROM reborn_component_graduation_receipts)",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(counts.get::<_, i64>(0), 1);
    assert_eq!(counts.get::<_, i64>(1), 0);
    assert_eq!(counts.get::<_, i64>(2), 0);
    let selected = program.inputs().instruction().retained_selection().unwrap();
    let stored = client.query_one("SELECT selection_bytes,selection_checksum,evidence_bytes FROM reborn_workflow_review_evidence WHERE evidence_id=$1", &[&workflow.id()]).await.unwrap();
    assert_eq!(stored.get::<_, &str>(0), selected.exact_bytes());
    assert_eq!(
        stored.get::<_, String>(1),
        format!("{:x}", Sha256::digest(selected.exact_bytes().as_bytes()))
    );
    assert_eq!(stored.get::<_, &str>(2), workflow.evidence());
    for statement in [
        "UPDATE reborn_workflow_review_evidence SET evidence_kind='behavior'",
        "DELETE FROM reborn_workflow_review_evidence",
        "TRUNCATE reborn_workflow_review_evidence",
    ] {
        assert_eq!(
            client.batch_execute(statement).await.unwrap_err().code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
    }

    let conflicting = Arc::new(
        StructuralReviewSet::prepare(Arc::new(
            InspectedRetainedProgram::inspect(RetainedProgram::Tools(program.clone()), worker())
                .await
                .unwrap(),
        ))
        .unwrap(),
    );
    let mut changed: Value = serde_json::from_str(conflicting.evidence(skill).unwrap()).unwrap();
    changed["succeeded"] = json!(false);
    let changed = changed.to_string();
    client.execute(
        "INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum) VALUES ($1,$2,$3)",
        &[&conflicting.references()[&skill], &changed, &format!("{:x}", Sha256::digest(changed.as_bytes()))],
    ).await.unwrap();
    let error = persist_structural_reviews(&rig.pool, conflicting.clone())
        .await
        .unwrap_err();
    assert!(matches!(error.failure, StructuralReviewFailure::Integrity));
    assert!(Arc::ptr_eq(&error.retained, &conflicting));
}

#[tokio::test]
async fn actual_commit_failure_retains_the_original_review_for_idempotent_recovery() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let program = retained_program::program(&store, false).await;
    let skill = program.bindings()["0:2"].skill().uuid;
    let reviews = Arc::new(
        StructuralReviewSet::prepare(Arc::new(
            InspectedRetainedProgram::inspect(RetainedProgram::Tools(program), worker())
                .await
                .unwrap(),
        ))
        .unwrap(),
    );
    let id = reviews.references()[&skill];
    let client = rig.pool.get().await.unwrap();
    client.batch_execute(
        "CREATE FUNCTION fail_actual_review_commit() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'actual validation persistence failure' USING ERRCODE='23514'; END; $$;
         CREATE CONSTRAINT TRIGGER actual_review_commit_fault AFTER INSERT
         ON reborn_component_review_evidence DEFERRABLE INITIALLY DEFERRED
         FOR EACH ROW EXECUTE FUNCTION fail_actual_review_commit()"
    ).await.unwrap();
    let error = persist_structural_reviews(&rig.pool, reviews.clone())
        .await
        .unwrap_err();
    let StructuralReviewFailure::Database(ref database) = error.failure else {
        panic!("actual PostgreSQL commit error required");
    };
    assert_eq!(
        database.code(),
        Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
    );
    assert!(Arc::ptr_eq(&error.retained, &reviews));
    assert_eq!(error.retained.references()[&skill], id);
    assert!(!format!("{error:?}").contains("actual validation persistence failure"));
    let actual: i64 = client
        .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        actual, 0,
        "actual failed commit must not publish partial evidence"
    );
    client
        .batch_execute(
            "DROP TRIGGER actual_review_commit_fault ON reborn_component_review_evidence",
        )
        .await
        .unwrap();
    persist_structural_reviews(&rig.pool, error.retained.clone())
        .await
        .unwrap();
    persist_structural_reviews(&rig.pool, error.retained.clone())
        .await
        .unwrap();
    let actual: String = client
        .query_one(
            "SELECT evidence_bytes FROM reborn_component_review_evidence WHERE evidence_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(actual, error.retained.evidence(skill).unwrap());
}

struct ObservedJsonPort {
    runtime: Arc<RetainedFirstPartyCapability>,
    prepared: Arc<brassclaw_engine::memory::retained_tools::RetainedToolProgram>,
    admitted: Arc<admission::Admitted>,
    task: TaskHandle,
}
#[async_trait]
impl RetainedToolPort for ObservedJsonPort {
    async fn dispatch(
        &self,
        invocation: RetainedToolInvocation<'_>,
        binding: &brassclaw_engine::memory::retained_tools::RetainedToolBinding,
        arguments: Value,
    ) -> PortAnswer {
        assert_eq!(invocation.task(), self.task);
        assert_eq!(
            self.prepared.bindings()[invocation.step_id()].tool(),
            binding.tool()
        );
        let record = self
            .admitted
            .admission
            .begin_tool_invocation(&self.prepared, invocation.step_id(), &arguments)
            .await
            .unwrap();
        let context = ExecutionContext::local_default(
            UserId::new("draft-operator").unwrap(),
            ExtensionId::new("draft-caller").unwrap(),
            RuntimeKind::FirstParty,
            TrustClass::FirstParty,
            CapabilitySet::default(),
            MountView::default(),
        )
        .unwrap();
        let actual = self
            .runtime
            .invoke(RuntimeCapabilityRequest::new(
                context,
                CapabilityId::new(binding.capability_id()).unwrap(),
                ResourceEstimate::default(),
                arguments,
                retained_kernel::trust(),
            ))
            .await
            .unwrap();
        let RuntimeCapabilityOutcome::Completed(actual) = actual else {
            panic!("actual JSON kernel completion required");
        };
        let answer = PortAnswer::Return {
            value: actual.output,
        };
        record.record_answer(&answer).await.unwrap();
        answer
    }
}
async fn exchange(transport: &TransportClient, command: WorkerCommand) -> ProcessSnapshot {
    transport
        .try_submit(command)
        .unwrap()
        .wait()
        .await
        .unwrap()
        .outcome
        .unwrap()
}
async fn progress(transport: &TransportClient, mut snapshot: ProcessSnapshot) -> ProcessSnapshot {
    while let Some(ProcessBoundary::ControlYield { key }) = snapshot.boundary {
        snapshot = exchange(transport, WorkerCommand::ResumeControl { key }).await;
    }
    snapshot
}

#[tokio::test]
async fn pure_logic_workflow_evidence_observes_values_and_rejects_reordered_steps() {
    use brassclaw_engine::memory::{
        retained_inputs::prepare_retained_unbound_program,
        retained_instruction::{WorkflowClass, compile_retained_recipe},
    };
    use pg_workflow_review::{
        ExpectedWorkflowTermination, WorkflowReview, WorkflowStepExpectation,
        persist_workflow_review,
    };
    use uuid::Uuid;

    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let (recipe, first, second) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let result = json!({"type":"object","allow_extra_fields":false,"fields":{"text":{"type":"string","required":true}}});
    let document = json!({"variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":["show %"],"variable_patterns":[{"name":"value","pattern":null,"description":null}]}],
        "step_descriptions":[{"desc_idx":0,"label":"pure logic handoff","yaml_source":"","steps":[
            {"stepnumber":1,"knowledge":"orchestrator","goal":"Prepare text","content":"","type":"component","include":[first]},
            {"stepnumber":2,"knowledge":"orchestrator","goal":"Consume prepared text","content":"","type":"component","include":[second]}]}],
        "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":{"value":{"type":"string","required":true,"checks":[]}},
            "steps":{"0:1":{"text":{"kind":"task_input","reference":"{{vars.value}}"}},
                "0:2":{"previous":{"kind":"result","step_id":"0:1","path":["text"]}}}}}});
    let mut refs = Vec::new();
    for (id, class, document, dependencies) in [
        (
            first,
            22,
            json!({"content":"result = {'text': inputs['text']}","includes":[],"dependency_registry":null,
            "input_contract":{"text":{"type":"string","required":true,"checks":[]}},"result_contract":result}),
            Vec::new(),
        ),
        (
            second,
            22,
            json!({"content":"result = {'text': inputs['previous'] + '!'}","includes":[],"dependency_registry":null,
            "input_contract":{"previous":{"type":"string","required":true,"checks":[]}},"result_contract":result}),
            Vec::new(),
        ),
        (recipe, 21, document, vec![first, second]),
    ] {
        let draft = ComponentRevisionDraft::from_json(
            &json!({"format":"component-revision/1","uuid":id,"class_code":class,
            "document":document,"dependencies":dependencies,"association":null})
            .to_string(),
        )
        .unwrap();
        refs.push(store.stage(&draft, 0).await.unwrap());
    }
    let selected = compile_retained_recipe(
        Arc::new(store.read_exact(&[recipe], &refs).await.unwrap()),
        recipe,
        "selected",
        WorkflowClass::Deterministic,
    )
    .unwrap();
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect(
            RetainedProgram::Unbound(Arc::new(
                prepare_retained_unbound_program(selected).unwrap(),
            )),
            worker(),
        )
        .await
        .unwrap(),
    );
    let structural = Arc::new(WorkflowReview::structural(inspected.clone()).unwrap());
    persist_workflow_review(&rig.pool, structural)
        .await
        .unwrap();
    for order in [["0:1", "0:2"], ["0:2", "0:1"]] {
        // This neutral fixture keeps a real root/task waiting while probing child
        // execution. It is constrained draft validation, not application startup
        // or a claimed production Recipe completion.
        let source = "import asyncio\nasync def validate():\n    task = await host.await_next_task(0)\n    host.enter_task(task['task_token'])\n    await host.validation_hold(task['task_token'])\nasyncio.run(validate())\n";
        let mut boot = support::boot(source);
        boot.bounds.workers = 1;
        boot.aliases.insert("validation_hold".into());
        let (mut owner, ready) = TransportOwner::start(
            worker(),
            boot,
            support::limits(),
            ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: 4 * 1024 * 1024,
                max_control_unclaimed: 8,
                max_control_reserved_frame_bytes: 4 * 1024 * 1024,
            },
        )
        .await
        .unwrap();
        let admitted = admission::reserve(rig.pool.clone(), "actual pure logic validation").await;
        let transport = owner.client();
        let snapshot = exchange(
            &transport,
            WorkerCommand::Admit {
                key: ready.work_waits[0].1,
                task: admitted.input,
            },
        )
        .await;
        let task = snapshot.admitted_task.unwrap();
        let root = progress(&transport, snapshot).await;
        let Some(ProcessBoundary::HostCall { name, key, .. }) = root.boundary else {
            panic!("actual validation wait required");
        };
        assert_eq!(name, "validation_hold");
        exchange(&transport, WorkerCommand::Defer { key }).await;
        let text = "'quoted'\\value\nÜ {{vars.value}} host.forbidden()";
        let expected = BTreeMap::from([
            (
                "0:1".into(),
                WorkflowStepExpectation {
                    inputs: json!({"text":text}),
                    tool: None,
                    result: ExpectedStepResult::Return(json!({"text":text})),
                },
            ),
            (
                "0:2".into(),
                WorkflowStepExpectation {
                    inputs: json!({"previous":text}),
                    tool: None,
                    result: ExpectedStepResult::Return(json!({"text":format!("{text}!")})),
                },
            ),
        ]);
        let mut execution =
            RetainedRecipeExecution::new_for_behavioral_validation(task, inspected.clone())
                .unwrap();
        assert!(matches!(
            WorkflowReview::behavior(
                &execution,
                &expected,
                ExpectedWorkflowTermination::CompletedSteps
            ),
            Err(StructuralReviewFailure::Observation)
        ));
        if order == ["0:1", "0:2"] {
            for step in order {
                let value = execution
                    .run_step(&transport, step, &expected[step].inputs, None)
                    .await
                    .unwrap();
                let ExpectedStepResult::Return(wanted) = &expected[step].result else {
                    panic!("expected actual value");
                };
                assert_eq!(&value, wanted);
            }
            assert!(execution.is_complete());
            let review = Arc::new(
                WorkflowReview::behavior(
                    &execution,
                    &expected,
                    ExpectedWorkflowTermination::CompletedSteps,
                )
                .unwrap(),
            );
            let value: Value = serde_json::from_str(review.evidence()).unwrap();
            assert_eq!(value["succeeded"], true);
            assert_eq!(value["report"]["semantic_approval"], false);
            assert_eq!(value["report"]["task_completion"], false);
            assert!(!review.evidence().contains("host.forbidden"));
            persist_workflow_review(&rig.pool, review).await.unwrap();
        } else {
            assert!(matches!(
                execution
                    .run_step(&transport, order[0], &expected[order[0]].inputs, None)
                    .await,
                Err(
                    brassclaw_engine::executor::retained_recipe::RetainedExecutionError::Invalid(
                        "step is not the next retained occurrence"
                    )
                )
            ));
            assert!(
                execution.latest_snapshot().is_none(),
                "no child allocation before retained-order verification"
            );
            assert!(execution.observations().is_empty());
            assert!(execution.completed_step_ids().next().is_none());
            assert!(!execution.is_complete());
            assert!(
                matches!(
                    WorkflowReview::behavior(
                        &execution,
                        &expected,
                        ExpectedWorkflowTermination::CompletedSteps
                    ),
                    Err(StructuralReviewFailure::Observation)
                ),
                "a rejected reordered request cannot supply passing workflow evidence"
            );
        }
        assert!(execution.host_answers().is_empty());
        owner.request_termination();
        let exit = owner.join().await.unwrap();
        assert!(exit.exit_status.is_some());
        assert!(exit.reap_error.is_none());
        assert!(exit.containment_error.is_none());
    }
}

#[tokio::test]
async fn behavioral_records_require_actual_execution_and_keep_failed_expectations_and_commit_evidence()
 {
    exercise_actual_behavior(None).await;
}

#[async_trait]
pub(crate) trait ReviewObserver: Send + Sync {
    async fn review(
        &self,
        pool: Arc<brassclaw_pg::PgPool>,
        skill: uuid::Uuid,
        q1: uuid::Uuid,
        behavior: uuid::Uuid,
        failed: bool,
    );
}

pub(crate) async fn exercise_actual_behavior(observer: Option<&dyn ReviewObserver>) {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    for invalid_output in [false, true] {
        let before_approvals: i64 = rig
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM reborn_skill_association_approvals",
                &[],
            )
            .await
            .unwrap()
            .get(0);
        let prepared = retained_program::program(&store, invalid_output).await;
        let skill = prepared.bindings()["0:2"].skill().uuid;
        let data = json!({"text":"'quotes'\\slashes\n Ü {{vars.data}} host.forbidden()"});
        let inputs = json!({"data":data.to_string()});
        let admitted =
            Arc::new(admission::reserve(rig.pool.clone(), inputs["data"].as_str().unwrap()).await);
        admitted.admission.check_and_start().await.unwrap();
        let definitions =
            include_str!("../../../crates/brassclaw_engine/orchestrator/global_mode.py")
                .strip_suffix("asyncio.run(_global_main())\n")
                .unwrap();
        let source = format!(
            "{definitions}\nasync def validate_draft():\n    task = await host.await_next_task(0)\n    host.enter_task(task['task_token'])\n    value = await _execute_recipe(task['task_token'], 'retained-draft', '0:1-0:E', {{'user_input': task['user_input']}})\n    await host.validation_result(task['task_token'], value)\nasyncio.run(validate_draft())\n"
        );
        let mut boot = support::boot(&source);
        boot.bounds.workers = 1;
        boot.aliases.insert("validation_result".into());
        let (mut owner, ready) = TransportOwner::start(
            support::worker(),
            boot,
            support::limits(),
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
        let snapshot = exchange(
            &transport,
            WorkerCommand::Admit {
                key: ready.work_waits[0].1,
                task: admitted.input.clone(),
            },
        )
        .await;
        let task = snapshot.admitted_task.unwrap();
        let root = progress(&transport, snapshot).await;
        let Some(ProcessBoundary::HostCall {
            key,
            name,
            args,
            kwargs,
        }) = root.boundary
        else {
            panic!("actual composition request required");
        };
        assert_eq!(name, "compose_orchestrator");
        assert!(kwargs.is_empty());
        assert_eq!(args[1], "retained-draft");
        admitted
            .admission
            .retain_recipe_selection(prepared.inputs().instruction())
            .await
            .unwrap();
        exchange(&transport, WorkerCommand::Defer { key }).await;
        let reference = uuid::Uuid::new_v4().to_string();
        let snapshot = exchange(&transport, WorkerCommand::Resolve { key, answer:PortAnswer::Return { value:json!({
            "ok":true,"program_ref":reference,
            "steps":prepared.program().steplist.iter().map(|s| json!({"step_id":s.step_id})).collect::<Vec<_>>(),
            "inputs":inputs,"flow":prepared.inputs().monty_flow().unwrap(),
        }) } }).await;
        let mut root = progress(&transport, snapshot).await;
        let inspected = Arc::new(
            InspectedRetainedProgram::inspect(RetainedProgram::Tools(prepared.clone()), worker())
                .await
                .unwrap(),
        );
        let mut execution =
            RetainedRecipeExecution::new_for_behavioral_validation(task, inspected).unwrap();
        let (runtime, _) = retained_kernel::runtime(prepared.bindings()["0:2"].tool().uuid);
        let port = ObservedJsonPort {
            runtime,
            prepared: prepared.clone(),
            admitted: admitted.clone(),
            task,
        };
        let mut expected = BTreeMap::from([(
            "0:2".into(),
            BehavioralExpectation {
                inputs: inputs.clone(),
                arguments: json!({"data":inputs["data"],"operation":"parse"}),
                answer: PortAnswer::Return {
                    value: data.clone(),
                },
                result: ExpectedStepResult::Return(data.clone()),
            },
        )]);
        assert!(
            matches!(
                BehavioralReviewSet::prepare(&execution, &expected),
                Err(StructuralReviewFailure::Observation)
            ),
            "no actual step, no behavior evidence"
        );
        for step in &prepared.program().steplist {
            let Some(ProcessBoundary::HostCall {
                key,
                name,
                args,
                kwargs,
            }) = root.boundary.take()
            else {
                panic!("actual Monty step request required");
            };
            assert_eq!(name, "run_program");
            assert_eq!(args[2], step.step_id);
            assert!(kwargs.is_empty());
            exchange(&transport, WorkerCommand::Defer { key }).await;
            let result = execution
                .run_step(&transport, &step.step_id, &args[3]["inputs"], Some(&port))
                .await;
            if invalid_output {
                assert!(result.is_err());
                assert!(
                    execution.observations()[&step.step_id].failure()
                        == Some(RetainedStepFailure::ResultContract)
                );
                break;
            }
            let value = result.unwrap();
            assert_eq!(value, data);
            let snapshot = exchange(
                &transport,
                WorkerCommand::Resolve {
                    key,
                    answer: PortAnswer::Return {
                        value: json!({"ok":true,"return_value":value}),
                    },
                },
            )
            .await;
            root = progress(&transport, snapshot).await;
        }
        if !invalid_output {
            let Some(ProcessBoundary::HostCall { name, args, .. }) = root.boundary else {
                panic!("actual validation result required");
            };
            assert_eq!(name, "validation_result");
            assert_eq!(args[1], data);
            expected.insert(
                "0:4".into(),
                BehavioralExpectation {
                    inputs: inputs.clone(),
                    arguments: json!({"data":inputs["data"],"operation":"parse"}),
                    answer: PortAnswer::Return {
                        value: data.clone(),
                    },
                    result: ExpectedStepResult::Return(data.clone()),
                },
            );
        }
        let workflow_expectations = expected
            .iter()
            .map(|(step, wanted)| {
                (
                    step.clone(),
                    pg_workflow_review::WorkflowStepExpectation {
                        inputs: wanted.inputs.clone(),
                        tool: Some((
                            wanted.arguments.clone(),
                            match &wanted.answer {
                                PortAnswer::Return { value } => PortAnswer::Return {
                                    value: value.clone(),
                                },
                                PortAnswer::DomainError { reason_kind } => {
                                    PortAnswer::DomainError {
                                        reason_kind: reason_kind.clone(),
                                    }
                                }
                                PortAnswer::TerminalError { reason_kind } => {
                                    PortAnswer::TerminalError {
                                        reason_kind: reason_kind.clone(),
                                    }
                                }
                            },
                        )),
                        result: ExpectedStepResult::Return(data.clone()),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let termination = if invalid_output {
            pg_workflow_review::ExpectedWorkflowTermination::FailedStep {
                step: "0:2".into(),
                failure: RetainedStepFailure::ResultContract,
            }
        } else {
            pg_workflow_review::ExpectedWorkflowTermination::CompletedSteps
        };
        if invalid_output {
            assert!(
                matches!(
                    pg_workflow_review::WorkflowReview::behavior(
                        &execution,
                        &workflow_expectations,
                        pg_workflow_review::ExpectedWorkflowTermination::CompletedSteps
                    ),
                    Err(StructuralReviewFailure::Observation)
                ),
                "a failed prefix cannot become complete-workflow evidence"
            );
        }
        let workflow = Arc::new(
            pg_workflow_review::WorkflowReview::behavior(
                &execution,
                &workflow_expectations,
                termination,
            )
            .unwrap(),
        );
        let evidence: Value = serde_json::from_str(workflow.evidence()).unwrap();
        assert_eq!(evidence["succeeded"], !invalid_output);
        assert_eq!(evidence["report"]["task_completion"], false);
        assert_eq!(
            evidence["report"]["execution_order"],
            if invalid_output {
                json!(["0:2"])
            } else {
                json!(["0:2", "0:4"])
            }
        );
        assert!(!workflow.evidence().contains("host.forbidden"));
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE FUNCTION fail_workflow_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'actual workflow commit failure' USING ERRCODE='23514'; END; $$; CREATE CONSTRAINT TRIGGER workflow_commit_fault AFTER INSERT ON reborn_workflow_review_evidence DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_workflow_commit()").await.unwrap();
        let error = pg_workflow_review::persist_workflow_review(&rig.pool, workflow.clone())
            .await
            .unwrap_err();
        assert!(matches!(
            error.failure,
            StructuralReviewFailure::Database(_)
        ));
        assert!(Arc::ptr_eq(&error.retained, &workflow));
        assert!(!format!("{error:?}").contains("actual workflow commit failure"));
        assert!(!client.query_one("SELECT EXISTS(SELECT 1 FROM reborn_workflow_review_evidence WHERE evidence_id=$1)", &[&workflow.id()]).await.unwrap().get::<_, bool>(0));
        client.batch_execute("DROP TRIGGER workflow_commit_fault ON reborn_workflow_review_evidence; DROP FUNCTION fail_workflow_commit()").await.unwrap();
        pg_workflow_review::persist_workflow_review(&rig.pool, error.retained.clone())
            .await
            .unwrap();
        pg_workflow_review::persist_workflow_review(&rig.pool, error.retained)
            .await
            .unwrap();
        let stored = client.query_one("SELECT selection_bytes,evidence_bytes FROM reborn_workflow_review_evidence WHERE evidence_id=$1", &[&workflow.id()]).await.unwrap();
        let journal: String = client.query_one("SELECT selection_bytes FROM brassclaw_monty_recipe_selections WHERE run_id=$1 AND recipe_id=$2", &[&admitted.context.run_id.as_uuid(), &prepared.inputs().instruction().recipe().uuid]).await.unwrap().get(0);
        assert_eq!(stored.get::<_, &str>(0), journal);
        assert_eq!(stored.get::<_, &str>(1), workflow.evidence());
        if invalid_output {
            let mut negative = workflow_expectations;
            negative.get_mut("0:2").unwrap().result =
                ExpectedStepResult::Failure(RetainedStepFailure::ResultContract);
            let reviewed = Arc::new(
                pg_workflow_review::WorkflowReview::behavior(
                    &execution,
                    &negative,
                    pg_workflow_review::ExpectedWorkflowTermination::FailedStep {
                        step: "0:2".into(),
                        failure: RetainedStepFailure::ResultContract,
                    },
                )
                .unwrap(),
            );
            assert_eq!(
                serde_json::from_str::<Value>(reviewed.evidence()).unwrap()["succeeded"],
                true
            );
            pg_workflow_review::persist_workflow_review(&rig.pool, reviewed)
                .await
                .unwrap();
        }
        drop(client);
        let reviews = Arc::new(BehavioralReviewSet::prepare(&execution, &expected).unwrap());
        assert_eq!(reviews.references().len(), 1);
        let actual: Value = serde_json::from_str(reviews.evidence(skill).unwrap()).unwrap();
        assert_eq!(actual["kind"], "behavior");
        assert_eq!(actual["succeeded"], !invalid_output);
        assert_eq!(actual["report"]["semantic_approval"], false);
        assert_eq!(actual["report"]["workflow_completion"], false);
        assert_eq!(
            actual["report"]["observations"].as_array().unwrap().len(),
            if invalid_output { 1 } else { 2 }
        );
        assert!(!reviews.evidence(skill).unwrap().contains("host.forbidden"));
        if invalid_output {
            expected.get_mut("0:2").unwrap().result =
                ExpectedStepResult::Failure(RetainedStepFailure::ResultContract);
            let failure_case =
                Arc::new(BehavioralReviewSet::prepare(&execution, &expected).unwrap());
            let observed: Value =
                serde_json::from_str(failure_case.evidence(skill).unwrap()).unwrap();
            assert_eq!(
                observed["succeeded"], true,
                "actual classified failure matches the explicit failure-case expectation"
            );
            persist_behavioral_reviews(&rig.pool, failure_case)
                .await
                .unwrap();
        }
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE FUNCTION fail_behavior_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'actual behavior commit failure' USING ERRCODE='23514'; END; $$; CREATE CONSTRAINT TRIGGER behavior_commit_fault AFTER INSERT ON reborn_component_review_evidence DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_behavior_commit()").await.unwrap();
        let error = persist_behavioral_reviews(&rig.pool, reviews.clone())
            .await
            .unwrap_err();
        assert!(matches!(
            error.failure,
            StructuralReviewFailure::Database(_)
        ));
        assert!(Arc::ptr_eq(&error.retained, &reviews));
        let id = reviews.references()[&skill];
        let absent:bool = client.query_one("SELECT NOT EXISTS(SELECT 1 FROM reborn_component_review_evidence WHERE evidence_id=$1)", &[&id]).await.unwrap().get(0);
        assert!(absent);
        client.batch_execute("DROP TRIGGER behavior_commit_fault ON reborn_component_review_evidence; DROP FUNCTION fail_behavior_commit()").await.unwrap();
        persist_behavioral_reviews(&rig.pool, error.retained.clone())
            .await
            .unwrap();
        persist_behavioral_reviews(&rig.pool, error.retained)
            .await
            .unwrap();
        let actual: String = client
            .query_one(
                "SELECT evidence_bytes FROM reborn_component_review_evidence WHERE evidence_id=$1",
                &[&id],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(actual, reviews.evidence(skill).unwrap());
        let approvals: i64 = client
            .query_one(
                "SELECT count(*) FROM reborn_skill_association_approvals",
                &[],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(approvals, before_approvals);
        let q1 = Arc::new(StructuralReviewSet::prepare(execution.inspected_program()).unwrap());
        persist_structural_reviews(&rig.pool, q1.clone())
            .await
            .unwrap();
        if let Some(observer) = observer {
            observer
                .review(
                    rig.pool.clone(),
                    skill,
                    q1.references()[&skill],
                    id,
                    invalid_output,
                )
                .await;
        }
        human_review_regression(
            &rig.pool,
            skill,
            q1.references()[&skill],
            id,
            invalid_output,
        )
        .await;
        let state = admitted
            .state
            .get_run_state(GetRunStateRequest {
                scope: admitted.context.scope.clone(),
                run_id: admitted.context.run_id,
            })
            .await
            .unwrap();
        assert!(
            !state.status.is_terminal(),
            "validation evidence does not manufacture product completion"
        );
        owner.request_termination();
        let exit = owner.join().await.unwrap();
        assert!(exit.exit_status.is_some());
        assert!(exit.reap_error.is_none());
        assert!(exit.containment_error.is_none());
        admitted
            .admission
            .settle(json!({"status":"failed","reason_kind":"recipe_execution_failed"}))
            .await
            .unwrap();
    }
}

async fn human_review_regression(
    pool: &brassclaw_pg::PgPool,
    skill: uuid::Uuid,
    q1: uuid::Uuid,
    behavior: uuid::Uuid,
    failed: bool,
) {
    use brassclaw_skills::association_review_store::{
        AssociationReviewStoreError, HumanAssociationDecision, prepare_human_association_review,
        record_human_association_approval,
    };
    use tokio_postgres::IsolationLevel;
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let result = prepare_human_association_review(&tx, skill, q1, &[behavior]).await;
    if failed {
        assert!(matches!(result, Err(AssociationReviewStoreError::Evidence)));
        tx.rollback().await.unwrap();
        return;
    }
    let review = result.unwrap();
    let checksum = review.checksum().to_owned();
    assert_eq!(review.view()["scope"], "one-tool-usage");
    assert_eq!(review.view()["activates_catalogue"], false);
    assert_eq!(review.view()["components"].as_array().unwrap().len(), 4);
    assert_eq!(review.view()["evidence"].as_array().unwrap().len(), 2);
    let selected_skill = review.view()["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["uuid"] == skill.to_string())
        .unwrap();
    let mut changed: Value =
        serde_json::from_str(selected_skill["revision_bytes"].as_str().unwrap()).unwrap();
    changed["document"]["review_note"] =
        json!("new unapproved draft must not alter the displayed combination");
    let draft = ComponentRevisionDraft::from_json(&changed.to_string()).unwrap();
    PgComponentRevisionStore::stage_in_transaction(
        &tx,
        &draft,
        selected_skill["version"].as_u64().unwrap(),
    )
    .await
    .unwrap();
    let approval = uuid::Uuid::new_v4();
    let q2 = uuid::Uuid::new_v4();
    let note = "Reviewed prose, exact executable JSON binding, typed input/result contracts and actual success cases.";
    let wrong = record_human_association_approval(
        &tx,
        &review,
        HumanAssociationDecision {
            approval_id: approval,
            q2_id: q2,
            actor: "authenticated-test-operator",
            semantic_review: note,
            reviewed_checksum: "changed-view",
        },
    )
    .await;
    assert!(matches!(wrong, Err(AssociationReviewStoreError::Evidence)));
    record_human_association_approval(
        &tx,
        &review,
        HumanAssociationDecision {
            approval_id: approval,
            q2_id: q2,
            actor: "authenticated-test-operator",
            semantic_review: note,
            reviewed_checksum: &checksum,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    // Exact recovery retains both identities/bytes. Newer draft heads never
    // substitute revisions after the human viewed this retained combination.
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let again = prepare_human_association_review(&tx, skill, q1, &[behavior])
        .await
        .unwrap();
    assert_eq!(again.checksum(), checksum);
    record_human_association_approval(
        &tx,
        &again,
        HumanAssociationDecision {
            approval_id: approval,
            q2_id: q2,
            actor: "authenticated-test-operator",
            semantic_review: note,
            reviewed_checksum: &checksum,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let counts = client
        .query_one(
            "SELECT
        (SELECT count(*) FROM reborn_skill_association_approvals WHERE approval_id=$1),
        (SELECT count(*) FROM reborn_component_review_evidence WHERE evidence_id=$2)",
            &[&approval, &q2],
        )
        .await
        .unwrap();
    assert_eq!(counts.get::<_, i64>(0), 1);
    assert_eq!(counts.get::<_, i64>(1), 1);
    let actual: String = client
        .query_one(
            "SELECT evidence_bytes FROM reborn_component_review_evidence WHERE evidence_id=$1",
            &[&q2],
        )
        .await
        .unwrap()
        .get(0);
    let actual: Value = serde_json::from_str(&actual).unwrap();
    assert_eq!(actual["report"]["actor"], "authenticated-test-operator");
    assert_eq!(actual["report"]["review_checksum"], checksum);
    assert_eq!(actual["report"]["semantic_review"], note);
    // A conflicting retry cannot overwrite an approved decision.
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let review = prepare_human_association_review(&tx, skill, q1, &[behavior])
        .await
        .unwrap();
    let result = record_human_association_approval(
        &tx,
        &review,
        HumanAssociationDecision {
            approval_id: approval,
            q2_id: q2,
            actor: "another-operator",
            semantic_review: note,
            reviewed_checksum: &checksum,
        },
    )
    .await;
    assert!(matches!(
        result,
        Err(AssociationReviewStoreError::Integrity)
    ));
    tx.rollback().await.unwrap();
    // Real deferred commit failure leaves neither half of a decision published.
    client.batch_execute("CREATE FUNCTION fail_q2_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'actual Q2 commit failure' USING ERRCODE='23514'; END; $$; CREATE CONSTRAINT TRIGGER q2_commit_fault AFTER INSERT ON reborn_skill_association_approvals DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_q2_commit()").await.unwrap();
    let recovery_approval = uuid::Uuid::new_v4();
    let recovery_q2 = uuid::Uuid::new_v4();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let review = prepare_human_association_review(&tx, skill, q1, &[behavior])
        .await
        .unwrap();
    record_human_association_approval(
        &tx,
        &review,
        HumanAssociationDecision {
            approval_id: recovery_approval,
            q2_id: recovery_q2,
            actor: "authenticated-test-operator",
            semantic_review: note,
            reviewed_checksum: &checksum,
        },
    )
    .await
    .unwrap();
    assert!(tx.commit().await.is_err());
    let row = client
        .query_one(
            "SELECT
        NOT EXISTS(SELECT 1 FROM reborn_skill_association_approvals WHERE approval_id=$1),
        NOT EXISTS(SELECT 1 FROM reborn_component_review_evidence WHERE evidence_id=$2)",
            &[&recovery_approval, &recovery_q2],
        )
        .await
        .unwrap();
    assert!(row.get::<_, bool>(0) && row.get::<_, bool>(1));
    client.batch_execute("DROP TRIGGER q2_commit_fault ON reborn_skill_association_approvals; DROP FUNCTION fail_q2_commit()").await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let review = prepare_human_association_review(&tx, skill, q1, &[behavior])
        .await
        .unwrap();
    record_human_association_approval(
        &tx,
        &review,
        HumanAssociationDecision {
            approval_id: recovery_approval,
            q2_id: recovery_q2,
            actor: "authenticated-test-operator",
            semantic_review: note,
            reviewed_checksum: &checksum,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
}
