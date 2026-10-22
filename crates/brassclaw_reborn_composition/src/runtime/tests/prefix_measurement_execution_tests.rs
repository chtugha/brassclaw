//! Development-only full Recipe observation. It exercises unapproved candidates
//! in a contained private task and cannot create bootstrap/activation evidence.
use crate::{
    bootstrap_recipe_flow, bootstrap_validation,
    global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe},
    runtime::RebornRuntime,
};
use async_trait::async_trait;
use brassclaw_engine::executor::{
    retained_recipe::{
        RetainedProgram, RetainedRecipeExecution, RetainedToolPort, port_answer_checksum,
        typed_value_checksum,
    },
    retained_source::InspectedRetainedProgram,
};
use brassclaw_monty_host::{
    process::PortAnswer,
    service::{PortFailure, TaskOutcome},
};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

struct Invocation {
    reference: String,
    inputs: Value,
    expected: Value,
    inspected: Arc<InspectedRetainedProgram>,
    tools: Arc<dyn RetainedToolPort>,
}
#[async_trait]
impl InternalRecipeSource for Invocation {
    fn name(&self) -> &str {
        "development-prefix-measurement"
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: self.inspected.clone(),
            inputs: Some(self.inputs.clone().into()),
            tools: Some(self.tools.clone()),
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        let bad = || PortFailure::new("measurement_receipt_invalid").unwrap();
        if !std::ptr::eq(selected, self.inspected.as_ref())
            || !execution.is_complete()
            || execution.host_answers().len() != 2
            || execution.transport_failure().is_some()
            || crate::prefix_generations::canonical(&self.expected).map_err(|_| bad())? != receipt
        {
            return Err(bad());
        }
        Ok(())
    }
}

pub(super) async fn execute_candidate(
    runtime: &Arc<RebornRuntime>,
    inspected: Arc<InspectedRetainedProgram>,
    native_input: &Value,
    reference: &str,
    original_template: &str,
    native_observation: &Value,
) {
    let pool = runtime.installed_catalogue.pool.clone();
    let fixture = crate::bootstrap_encoding_fixture::Fixture::retain(pool.clone())
        .await
        .unwrap();
    let public_store = crate::prefix_model_artifacts::PgPrefixModelArtifacts::new(
        pool.clone(),
        runtime.webui_tenant_id().into(),
    );
    let original = public_store
        .get(
            native_input["specification"].as_str().unwrap(),
            native_input["artifact_set"].as_str().unwrap(),
        )
        .await
        .unwrap();
    // Retain the same bytes in the private test task's namespace. No alias or
    // cross-namespace lookup bypass is introduced in the production backend.
    let private_store = crate::prefix_model_artifacts::PgPrefixModelArtifacts::new(
        pool.clone(),
        crate::bootstrap_validation::NAMESPACE.into(),
    );
    let assets = private_store
        .retain_with_origin(
            brassclaw_product_workflow::RetainPrefixModelArtifactsRequest {
                specification: fixture.specification.clone(),
                artifact_set_bytes: original.artifact_set_bytes,
            },
            crate::prefix_model_artifacts::ArtifactImportOrigin::SystemSeedFixture {
                actor: "contained-development-fixture".into(),
                producer: crate::bootstrap_encoding_fixture::PRODUCER.into(),
                source_checksum: native_input["artifact_set"].as_str().unwrap().into(),
            },
        )
        .await
        .unwrap();
    let request = json!({"reference":reference,"original_template":original_template,"probes_json":native_input["probes_json"],"specification":fixture.specification,"artifact_set":assets.id,
        "limits_ref":"contained-development-context-fixture/1","context_window_tokens":131072,"reserved_output_tokens":8192,"reserved_additional_input_tokens":2048});
    let inputs = json!({"request_json":crate::prefix_generations::canonical(&request).unwrap()});
    let report = &native_observation["report"];
    let largest = report["probes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["token_count"].as_u64().unwrap())
        .max()
        .unwrap();
    let required = largest + 8192 + 2048;
    let expected = json!({"format":"brassclaw-prefix-capacity-comparison/1","qualification":"calculation-only; not approval or target verification",
        "specification":fixture.specification,"artifact_set":assets.id,"limits_ref":"contained-development-context-fixture/1",
        "template_sha256":report["chat_template_sha256"],"worker_checksum":native_observation["worker_checksum"],"preparation_revision":native_observation["preparation_revision"],
        "context_window_tokens":131072,"largest_probe_tokens":largest,"reserved_output_tokens":8192,"reserved_additional_input_tokens":2048,"required_tokens":required,
        "fits":required<=131072,"remaining_tokens":131072u64.saturating_sub(required),"shortfall_tokens":required.saturating_sub(131072),"common_prefix_tokens":report["common_prefix_tokens"],"cache_qualification":"unverified","model_quality":"unmeasured"});
    let reference = format!("development-prefix-measurement:{}", Uuid::new_v4());
    let admitted = Box::pin(bootstrap_validation::admit(
        pool.clone(),
        "development-prefix-measurement",
        &reference,
    ))
    .await
    .unwrap();
    let approvals_before: i64 = pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
        .await
        .unwrap()
        .get(0);
    let RetainedProgram::Tools(program) = inspected.program() else {
        panic!("Tool program required");
    };
    let tools = crate::installed_monty_catalogue::bootstrap_measurement_tools(
        pool.clone(),
        runtime.installed_catalogue.kernel.clone(),
        admitted.host.clone(),
        admitted.admission.clone(),
        program.clone(),
        false,
    )
    .unwrap();
    let root = crate::global_root_seed::retain_packaged_global_root(
        pool.clone(),
        crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
    )
    .await
    .unwrap();
    let observed = bootstrap_recipe_flow::execute(
        &admitted,
        bootstrap_recipe_flow::Request::contained(
            runtime.installed_catalogue.worker.clone(),
            inspected.preparation_limits(),
            &root,
            Arc::new(Invocation {
                reference,
                inputs,
                expected: expected.clone(),
                inspected: inspected.clone(),
                tools,
            }),
        )
        .unwrap(),
    )
    .await
    .unwrap();
    assert!(
        matches!(
            observed.receipt.outcome,
            TaskOutcome::InternalCompleted { .. }
        ),
        "{:?}",
        observed.receipt.outcome
    );
    assert_eq!(observed.report["root_completed"], true);
    assert_eq!(observed.report["model_exchange_count"], 0);
    assert!(observed.report["reply"].is_null());
    assert_eq!(observed.report["semantic_approval"], false);
    let fingerprints = observed
        .ports
        .observe_internal_execution(|execution| {
            assert!(execution.is_complete());
            assert!(execution.transport_failure().is_none());
            let order: Vec<_> = [1, 2, 3, 5, 7, 8, 9, 10].map(|n| format!("0:{n}")).into();
            assert_eq!(execution.observation_order(), Some(order.as_slice()));
            assert_eq!(execution.observations().len(), 8);
            for proof in execution.observations().values() {
                assert!(proof.settled());
                assert!(proof.observation_error().is_none());
            }
            assert_eq!(execution.completed_step_ids().collect::<Vec<_>>(), order);
            assert_eq!(execution.host_answers().len(), 2);
            let PortAnswer::Return { value: digests } = &execution.host_answers()[0].1 else {
                panic!("actual digest result required");
            };
            assert_eq!(
                digests["digests"],
                json!([
                    crate::bootstrap_encoding_fixture::checksum(
                        native_input["template_utf8"].as_str().unwrap().as_bytes()
                    ),
                    crate::bootstrap_encoding_fixture::checksum(original_template.as_bytes()),
                ])
            );
            let PortAnswer::Return { value: encoded } = &execution.host_answers()[1].1 else {
                panic!("actual model observation required");
            };
            let encoded: Value = serde_json::from_str(encoded.as_str().unwrap()).unwrap();
            assert_eq!(
                encoded["report"], native_observation["report"],
                "the complete ordered report must equal the independent HF-verified observation"
            );
            assert_eq!(encoded["specification"], fixture.specification);
            assert_eq!(encoded["artifact_set"], assets.id);
            execution
                .host_answers()
                .iter()
                .map(|(step, answer)| {
                    let proof = &execution.observations()[step];
                    assert_eq!(
                        proof.answer_checksum(),
                        Some(port_answer_checksum(answer).unwrap())
                    );
                    (
                        step.clone(),
                        (
                            proof.arguments_checksum().unwrap(),
                            proof.answer_checksum().unwrap(),
                        ),
                    )
                })
                .collect::<std::collections::BTreeMap<_, _>>()
        })
        .await
        .unwrap();
    assert!(admitted.host.finalized_reply_ref().is_none());
    assert!(observed.exit.failure.is_none());
    assert!(observed.exit.pending_admission.is_none());
    assert!(observed.exit.queued_admissions.is_empty());
    let exit = observed.exit.transport.unwrap();
    assert!(exit.exit_status.is_some());
    assert!(exit.containment_error.is_none());
    assert!(exit.reap_error.is_none());
    assert!(exit.shutdown_failure.is_none());
    bootstrap_validation::settle_closed(&admitted)
        .await
        .unwrap();
    let client = pool.get().await.unwrap();
    let approvals_after: i64 = client
        .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        approvals_before, approvals_after,
        "candidate execution cannot manufacture approval evidence"
    );
    let rows=client.query("SELECT step_id,recipe_id,selection_bytes,selection_checksum,arguments_bytes,answer_bytes,phase,attempt_count FROM brassclaw_monty_tool_invocations WHERE run_id=$1 ORDER BY step_id",&[&admitted.attempt.run_id.as_uuid()]).await.unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        let step: String = row.get(0);
        assert!(matches!(step.as_str(), "0:5" | "0:7"));
        assert_eq!(
            row.get::<_, Uuid>(1),
            program.inputs().instruction().recipe().uuid
        );
        let selected =
            crate::pg_monty_admission::PgMontyAdmission::invocation_selection(program, &step)
                .unwrap();
        assert_eq!(row.get::<_, String>(2), selected);
        assert_eq!(
            row.get::<_, String>(3),
            crate::bootstrap_evidence::checksum(&selected)
        );
        let arguments: Value = serde_json::from_str(&row.get::<_, String>(4)).unwrap();
        let answer: PortAnswer =
            serde_json::from_str(&row.get::<_, Option<String>>(5).unwrap()).unwrap();
        assert_eq!(row.get::<_, String>(6), "answered");
        assert_eq!(row.get::<_, i16>(7), 1);
        assert_eq!(
            fingerprints[&step],
            (
                typed_value_checksum(&arguments).unwrap(),
                port_answer_checksum(&answer).unwrap()
            )
        );
    }
    eprintln!(
        "full measurement candidate: eight executable steps, two real native dispatches, exact internal receipt, context fit={}; no approval or activation",
        expected["fits"]
    );
}
