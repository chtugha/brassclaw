//! Real installed candidate selection and closed global Recipe execution.
//! Source authority comes from the existing capture owner; this is not ordinary
//! admission, semantic approval, canonical policy resolution or serving.
use super::*;
use crate::{
    bootstrap_validation,
    global_recipe_ports::{GlobalRecipePorts, InternalRecipeSource, SelectedMontyRecipe},
};
use async_trait::async_trait;
use brassclaw_engine::executor::{
    retained_recipe::{
        RetainedProgram, RetainedRecipeExecution, RetainedToolPort,
        port_answer_checksum_with_limits, typed_value_checksum_with_limits,
    },
    retained_source::InspectedRetainedProgram,
};
use brassclaw_monty_host::{
    process::PortAnswer,
    service::{PortFailure, TaskEntry, TaskInput, TaskOutcome},
};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

struct Invocation {
    reference: String,
    input: Value,
    inspected: Arc<InspectedRetainedProgram>,
    tools: Arc<dyn RetainedToolPort>,
    pool: Arc<brassclaw_pg::PgPool>,
    run: Uuid,
    budget: Option<(u64, &'static str)>,
}
#[async_trait]
impl InternalRecipeSource for Invocation {
    fn name(&self) -> &str {
        "closed-captured-design-selection"
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: self.inspected.clone(),
            inputs: Some(self.input.clone().into()),
            tools: Some(self.tools.clone()),
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        assert!(std::ptr::eq(selected, self.inspected.as_ref()));
        let mut actual: Value = serde_json::from_str(receipt).unwrap();
        if let Some((effective, status)) = self.budget {
            assert_eq!(
                actual["format"],
                "brassclaw-budgeted-prefix-design-selection/1"
            );
            let planned = &actual["budget"];
            assert_eq!(planned["effective_prefix_budget"], effective);
            assert_eq!(planned["status"], status);
            for name in [
                "policy_ref",
                "max_prefix_tokens",
                "max_prompt_tokens",
                "context_window_tokens",
                "requested_prefix_tokens",
                "reserved_output_tokens",
                "additional_framing_tokens",
            ] {
                assert_eq!(planned[name], self.input[name]);
            }
            actual =
                serde_json::from_str(actual["captured_receipt_bytes"].as_str().unwrap()).unwrap();
        }
        assert_eq!(
            actual["format"],
            "brassclaw-captured-prefix-design-selection/1"
        );
        assert_eq!(actual["snapshot_bytes"], self.input["snapshot_bytes"]);
        let comparison: Value =
            serde_json::from_str(actual["comparison_bytes"].as_str().unwrap()).unwrap();
        assert_eq!(comparison["design_bytes"], self.input["design_bytes"]);
        let floor: Value =
            serde_json::from_str(comparison["floor_bytes"].as_str().unwrap()).unwrap();
        assert_eq!(floor["profile"], "base-prompt");
        assert!(
            floor["required_coverage"]
                .as_array()
                .unwrap()
                .iter()
                .any(|source| source["path"] == "skills.md")
        );
        let captured: Value =
            serde_json::from_str(self.input["snapshot_bytes"].as_str().unwrap()).unwrap();
        let sources: Value =
            serde_json::from_str(comparison["sources_bytes"].as_str().unwrap()).unwrap();
        let originals = captured["sources"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|source| source["disposition"] == "included")
            .collect::<Vec<_>>();
        assert_eq!(
            sources["sources"].as_array().unwrap().len(),
            originals.len()
        );
        for source in sources["sources"].as_array().unwrap() {
            let original = originals
                .iter()
                .find(|original| original["path"] == source["path"])
                .unwrap();
            assert_eq!(source["text"], original["text"]);
            assert_eq!(
                source["sha256"],
                crate::bootstrap_evidence::checksum(original["text"].as_str().unwrap())
            );
        }
        let order: &[&str] = if self.budget.is_some() {
            &[
                "2:1", "2:2", "2:4", "2:5", "2:6", "2:7", "2:9", "2:11", "2:12", "2:13",
            ]
        } else {
            &["1:1", "1:3", "1:4", "1:5", "1:6", "1:8", "1:10", "1:11"]
        };
        assert!(execution.is_complete());
        assert!(
            execution
                .observation_order()
                .unwrap()
                .iter()
                .map(String::as_str)
                .eq(order.iter().copied())
        );
        assert_eq!(execution.host_answers().len(), 3);
        let RetainedProgram::Tools(program) = selected.program() else {
            panic!("actual Tool program required")
        };
        let client = self.pool.get().await.unwrap();
        let rows = client.query("SELECT step_id,selection_bytes,arguments_bytes,answer_bytes,phase,attempt_count FROM brassclaw_monty_tool_invocations WHERE run_id=$1", &[&self.run]).await.unwrap();
        assert_eq!(rows.len(), 3);
        // Match the original step identity; SQL row order is not execution order.
        for (step, answer) in execution.host_answers() {
            let row = rows
                .iter()
                .find(|row| row.get::<_, String>(0) == *step)
                .unwrap();
            assert_eq!(
                row.get::<_, String>(1),
                crate::pg_monty_admission::PgMontyAdmission::invocation_selection(program, step)
                    .unwrap()
            );
            assert_eq!(row.get::<_, String>(4), "answered");
            assert_eq!(row.get::<_, i16>(5), 1);
            let proof = &execution.observations()[step];
            let limits = proof.data_limits();
            let args: Value = serde_json::from_str(&row.get::<_, String>(2)).unwrap();
            let recorded: PortAnswer =
                serde_json::from_str(row.get::<_, Option<String>>(3).as_deref().unwrap()).unwrap();
            let checksum = port_answer_checksum_with_limits(answer, limits).unwrap();
            assert_eq!(
                port_answer_checksum_with_limits(&recorded, limits).unwrap(),
                checksum
            );
            assert_eq!(proof.answer_checksum(), Some(checksum));
            assert_eq!(
                proof.arguments_checksum(),
                Some(typed_value_checksum_with_limits(&args, limits).unwrap())
            );
            assert!(matches!(answer, PortAnswer::Return { .. }));
        }
        let selection: Value =
            serde_json::from_str(actual["selection_bytes"].as_str().unwrap()).unwrap();
        assert_eq!(
            selection["prefix_budget"],
            self.budget
                .map(|(effective, _)| json!(effective))
                .unwrap_or_else(|| self.input["prefix_budget"].clone())
        );
        assert_eq!(selection["policy_ref"], self.input["policy_ref"]);
        assert_eq!(
            selection["sources_sha256"],
            crate::bootstrap_evidence::checksum(comparison["sources_bytes"].as_str().unwrap())
        );
        let (_, PortAnswer::Return { value }) = execution
            .host_answers()
            .iter()
            .find(|(step, _)| {
                step == if self.budget.is_some() {
                    "2:11"
                } else {
                    "1:10"
                }
            })
            .unwrap()
        else {
            panic!("actual encoding answer required")
        };
        assert_eq!(&selection["observation_bytes"], value.as_value());
        if !selection["selected"].is_null() {
            let units = selection["selected"]["units"].as_array().unwrap();
            assert!(
                comparison["required_units"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|unit| units.contains(unit))
            );
            assert!(
                selection["dispositions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|unit| unit["disposition"] == "selected")
            );
        } else {
            assert!(
                selection["dispositions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|unit| unit["disposition"] == "not-selected"
                        && unit["reason"] == "no-complete-candidate-fits")
            );
        }
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn installed_captured_design_recipe_preserves_sources_and_enforces_independent_floor() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("captured-design-owner", directory.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();
    assert!(
        !runtime
            .webui_monty_settings_store()
            .get("default", "default")
            .await
            .unwrap()
            .token_budgets_enabled,
        "canonical model ceilings must apply with discretionary budgets disabled"
    );
    let catalogue = &runtime.installed_catalogue;
    let pool = catalogue.pool.clone();
    let candidates = &catalogue._design_selection_candidates;
    let generation = catalogue.active.read().await.generation.to_string();
    let public = catalogue.export_sources(&generation).await.unwrap();
    assert!(
        public
            .components
            .iter()
            .all(|component| component.reference.uuid
                != crate::installed_monty_catalogue::id("prefix-design-selection:recipe")
                    .to_string()),
        "retention must not activate the candidate for ordinary routing"
    );
    assert_eq!(candidates.len(), 3);
    for (candidate, expected) in candidates.iter().zip([2, 3, 3]) {
        let RetainedProgram::Tools(program) = candidate.program() else {
            panic!("actual Tool program required")
        };
        assert_eq!(program.bindings().len(), expected);
        for uuid in [
            "prefix-source-compiler:recipe",
            "prefix-source-compiler:receipt",
        ]
        .map(crate::installed_monty_catalogue::id)
        {
            assert!(
                !program
                    .inputs()
                    .instruction()
                    .snapshot()
                    .revisions()
                    .contains_key(&uuid),
                "reusing source definitions must not retain unrelated workflows"
            );
        }
        for uuid in [
            "prefix-source-compiler:units",
            "prefix-source-compiler:renderer",
            "prefix-source-compiler:library",
            "prefix-source-compiler:prepare",
        ]
        .map(crate::installed_monty_catalogue::id)
        {
            let original = &catalogue
                ._source_compiler_candidates
                .source
                .program()
                .inputs()
                .instruction()
                .snapshot()
                .revisions()[&uuid];
            assert_eq!(
                program.inputs().instruction().snapshot().revisions()[&uuid].reference(),
                original.reference()
            );
        }
    }
    let snapshot = catalogue
        .capture_core_prefix_sources("base-prompt")
        .await
        .unwrap();
    let captured: Value = serde_json::from_str(&snapshot).unwrap();
    let selectors = captured["sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|source| source["disposition"] == "included")
        .map(|source| json!({"path":source["path"],"match":"exact","first_lines":[]}))
        .collect::<Vec<_>>();
    let rules = json!({"format":"brassclaw-prefix-generation-rules/1","profile":"base-prompt","header":"Complete captured reference; no accepted model or serving claim.","required_coverage":selectors,
        "bands":[{"minimum_tokens":0,"maximum_tokens":1,"groups":[{"id":"contracts","required":true,"priority":0,"dependencies":[],"selectors":selectors}]}]});
    let fixture = crate::bootstrap_encoding_fixture::Fixture::retain(pool.clone())
        .await
        .unwrap();
    // Real WordLevel tokenizer with no pre-tokenizer: every nonempty complete
    // reference is one UNK token. This establishes an independent exact 0/1
    // fixture boundary, not Ornith capacity or a copied production limit.
    let mut tokenizer: Value =
        serde_json::from_str(&crate::bootstrap_encoding_fixture::tokenizer()).unwrap();
    tokenizer["pre_tokenizer"] = Value::Null;
    let files: serde_json::Map<String, Value> = [
        ("config.json", "{}".into()),
        ("tokenizer.json", tokenizer.to_string()),
        (
            "tokenizer_config.json",
            json!({"chat_template":crate::bootstrap_encoding_fixture::TEMPLATE}).to_string(),
        ),
    ]
    .into_iter()
    .map(|(name, bytes): (&str, String)| {
        (
            name.into(),
            json!({"encoding":"base64","data":STANDARD.encode(bytes.as_bytes())}),
        )
    })
    .collect();
    let assets = crate::prefix_model_artifacts::PgPrefixModelArtifacts::new(
        pool.clone(),
        bootstrap_validation::NAMESPACE.into(),
    )
    .retain_with_origin(
        brassclaw_product_workflow::RetainPrefixModelArtifactsRequest {
            specification: fixture.specification.clone(),
            artifact_set_bytes:
                json!({"format":"brassclaw-prefix-model-artifact-import/1","artifacts":files})
                    .to_string(),
        },
        crate::prefix_model_artifacts::ArtifactImportOrigin::Operator {
            actor: "closed-design-fixture".into(),
        },
    )
    .await
    .unwrap();
    let policy_store = crate::prefix_model_designs::PgPrefixModelDesigns::new(
        pool.clone(),
        bootstrap_validation::NAMESPACE.into(),
    );
    let policy_repo = crate::pg_provider_repo::PgProviderRepo::new(
        pool.as_ref().clone(),
        bootstrap_validation::NAMESPACE,
    );
    let mut canonical = policy_repo
        .replace_revision(
            serde_json::from_value(json!({
                "id":"captured-budget-model", "aliases":[], "protocol":"open_ai_completions",
                "default_base_url":"http://127.0.0.1:1/v1", "base_url_required":true,
                "api_key_required":false, "model_env":"UNUSED_BUDGET_MODEL_FIXTURE",
                "default_model":"budget-model", "description":"Exact native budget fixture",
                "context_window_tokens":5,
                "model_token_limits":{"budget-model":{"max_prefix_tokens":3,"max_prompt_tokens":2}}
            }))
            .unwrap(),
            None,
        )
        .await
        .unwrap();
    let global = runtime.global_monty_owner.client();
    let identity = global.root_identity();
    for case in [
        "fits",
        "blocked",
        "weakened",
        "hidden-weakened-band",
        "budget-context",
        "budget-request",
        "budget-output",
        "budget-framing",
        "budget-policy",
        "budget-disabled",
    ] {
        let mut design = rules.clone();
        let weak = selectors
            .iter()
            .filter(|selector| selector["path"] != "skills.md")
            .cloned()
            .collect::<Vec<_>>();
        if case == "weakened" {
            design["required_coverage"] = json!(weak);
            design["bands"][0]["groups"][0]["selectors"] = json!(weak);
        } else if case == "hidden-weakened-band" {
            design["required_coverage"] = json!(weak);
            let mut hidden = design["bands"][0].clone();
            hidden["minimum_tokens"] = json!(2);
            hidden["maximum_tokens"] = json!(2);
            hidden["groups"][0]["selectors"] = json!(weak);
            design["bands"].as_array_mut().unwrap().push(hidden);
        }
        let mut input = json!({"snapshot_bytes":snapshot,"design_bytes":design.to_string(),"prefix_budget":if case=="blocked" {0} else {1},"policy_ref":"closed-reference-budget-fixture-only","special_tokens":[],"specification":fixture.specification,"artifact_set":assets.id});
        let budget = if case.starts_with("budget-") {
            if matches!(case, "budget-policy" | "budget-disabled") {
                let mut changed = canonical.definition.clone();
                changed
                    .model_token_limits
                    .0
                    .get_mut("budget-model")
                    .unwrap()
                    .max_prefix_tokens = if case == "budget-disabled" { 0 } else { 1 };
                canonical = policy_repo
                    .replace_revision(changed, Some(canonical.revision))
                    .await
                    .unwrap();
            }
            let model = policy_store
                .capture(
                    brassclaw_product_workflow::CapturePrefixModelRequest {
                        provider_id: canonical.definition.id.clone(),
                        expected_provider_revision: canonical.revision,
                        model: canonical.definition.default_model.clone(),
                    },
                    vec![brassclaw_product_workflow::PrefixModelTarget {
                        provider_id: canonical.definition.id.clone(),
                        provider_revision: canonical.revision,
                        model: canonical.definition.default_model.clone(),
                        consumer: "configured".into(),
                        configured_context_window: canonical.definition.context_window_tokens,
                    }],
                )
                .await
                .unwrap();
            let (_, planned) = policy_store
                .read_model_input(
                    &model.id,
                    Some(crate::prefix_model_designs::PrefixBuildReservations {
                        requested_prefix_tokens: if case == "budget-request" { 0 } else { 3 },
                        reserved_output_tokens: match case {
                            "budget-context" => 2,
                            "budget-output" => 4,
                            _ => 1,
                        },
                        additional_framing_tokens: if case == "budget-framing" { 2 } else { 0 },
                    }),
                )
                .await
                .unwrap();
            let specific_assets = crate::prefix_model_artifacts::PgPrefixModelArtifacts::new(
                pool.clone(), bootstrap_validation::NAMESPACE.into(),
            ).retain_with_origin(brassclaw_product_workflow::RetainPrefixModelArtifactsRequest {
                specification:model.id.clone(),
                artifact_set_bytes:json!({"format":"brassclaw-prefix-model-artifact-import/1","artifacts":files}).to_string(),
            },crate::prefix_model_artifacts::ArtifactImportOrigin::Operator {actor:"closed-budget-fixture".into()}).await.unwrap();
            input.as_object_mut().unwrap().remove("prefix_budget");
            input
                .as_object_mut()
                .unwrap()
                .extend(planned.unwrap().as_object().unwrap().clone());
            input["specification"] = json!(model.id);
            input["artifact_set"] = json!(specific_assets.id);
            Some((
                if matches!(case, "budget-context" | "budget-policy") {
                    1
                } else {
                    0
                },
                match case {
                    "budget-disabled" => "prefix-disabled",
                    "budget-output" => "reservation-blocked",
                    _ => "planning-ready",
                },
            ))
        } else {
            None
        };
        let reference = format!("closed-captured-design:{case}:{}", Uuid::new_v4());
        let admitted =
            bootstrap_validation::admit(pool.clone(), "captured-design-owner", &reference, None)
                .await
                .unwrap();
        let selected = candidates[if budget.is_some() { 2 } else { 1 }].clone();
        let RetainedProgram::Tools(program) = selected.program() else {
            panic!("actual Tool program required")
        };
        let tools = crate::installed_monty_catalogue::design_selection_candidate_tools(
            pool.clone(),
            catalogue.kernel.clone(),
            admitted.host.clone(),
            admitted.admission.clone(),
            program.clone(),
        )
        .unwrap();
        // This complete captured corpus exceeds the small-fixture helper's
        // physical frame. Use the already-running singleton and acknowledged
        // instance capacity instead; this is still private candidate admission.
        let ports = Arc::new(GlobalRecipePorts::for_internal(
            admitted.host.clone(),
            admitted.admission.clone(),
            Arc::new(Invocation {
                reference: reference.clone(),
                input,
                inspected: selected,
                tools,
                pool: pool.clone(),
                run: admitted.attempt.run_id.as_uuid(),
                budget,
            }),
            Some(runtime.global_monty_owner.ownership_check()),
            true,
            runtime
                .monty_settings_owner
                .store()
                .recipe_capacity_source(),
        ));
        let field = |name: &str| admitted.input[name].as_str().unwrap().to_owned();
        let ticket = global
            .submit_when_available(
                TaskInput {
                    entry: TaskEntry::InternalRecipe {
                        name: "closed-captured-design-selection".into(),
                    },
                    conversation_id: field("conversation_id"),
                    message_id: field("message_id"),
                    turn_id: field("turn_id"),
                    run_id: field("run_id"),
                    user_input: reference,
                    history: vec![],
                },
                ports.clone(),
            )
            .await
            .unwrap();
        let receipt = ticket.wait().await.unwrap();
        let report = ports.settlement_report(&receipt).await.unwrap();
        assert_eq!(global.root_identity(), identity);
        assert_eq!(report["model_exchange_count"], 0);
        assert!(report["reply"].is_null());
        assert_eq!(report["semantic_approval"], false);
        let success = !matches!(case, "weakened" | "hidden-weakened-band");
        if success && !matches!(receipt.outcome, TaskOutcome::InternalCompleted { .. }) {
            // Preserve the actual closed fixture's original data and private
            // worker evidence. Raw commands/VM diagnostics never enter logs.
            // This observer cannot change, retry or advance the failed step.
            ports
                .observe_internal_execution(|execution| {
                    use brassclaw_engine::executor::retained_recipe::RetainedTransportEvidence;
                    let evidence = tempfile::tempdir().unwrap().keep();
                    std::fs::write(evidence.join("snapshot.json"), &snapshot).unwrap();
                    std::fs::write(
                        evidence.join("report.json"),
                        serde_json::to_vec(&report).unwrap(),
                    )
                    .unwrap();
                    if let Some(state) = execution.latest_snapshot() {
                        std::fs::write(
                            evidence.join("worker-snapshot.json"),
                            serde_json::to_vec(state).unwrap(),
                        )
                        .unwrap();
                    }
                    if let Some(RetainedTransportEvidence::Process { error, .. }) =
                        execution.transport_failure()
                    {
                        std::fs::write(
                            evidence.join("worker-failure-kind.txt"),
                            format!("{:?}", error.kind),
                        )
                        .unwrap();
                        if let Some(diagnostic) = &error.diagnostic {
                            std::fs::write(evidence.join("worker-diagnostic.txt"), diagnostic)
                                .unwrap();
                        }
                        if let Some(state) = &error.snapshot {
                            std::fs::write(
                                evidence.join("failed-worker-snapshot.json"),
                                serde_json::to_vec(state).unwrap(),
                            )
                            .unwrap();
                        }
                    }
                    eprintln!(
                        "closed captured-design failure evidence: {}",
                        evidence.display()
                    );
                })
                .await
                .unwrap();
        }
        assert_eq!(
            matches!(receipt.outcome, TaskOutcome::InternalCompleted { .. }),
            success,
            "{case}: {:?}",
            report
        );
        if success {
            let mut receipt: Value =
                serde_json::from_str(report["internal_receipt"].as_str().unwrap()).unwrap();
            if budget.is_some() {
                receipt = serde_json::from_str(receipt["captured_receipt_bytes"].as_str().unwrap())
                    .unwrap();
            }
            let blocked = budget.is_some_and(|(effective, _)| effective == 0) || case == "blocked";
            let selection: Value =
                serde_json::from_str(receipt["selection_bytes"].as_str().unwrap()).unwrap();
            assert_eq!(
                selection["status"],
                if blocked {
                    "capacity-blocked"
                } else {
                    "reference-selected"
                }
            );
            assert_eq!(selection["mandatory_candidate_tokens"], 1);
            assert_eq!(selection["selected"].is_null(), blocked);
            assert_eq!(receipt["snapshot_bytes"], snapshot);
        } else {
            ports
                .observe_internal_execution(|execution| {
                    assert_eq!(execution.failed_step_id(), Some("1:5"));
                    assert!(!execution.observations().contains_key("1:6"));
                })
                .await
                .unwrap();
        }
        let calls: i64 = pool.get().await.unwrap().query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1 AND phase='answered' AND attempt_count=1", &[&admitted.attempt.run_id.as_uuid()]).await.unwrap().get(0);
        assert_eq!(calls, if success { 3 } else { 1 });
        assert!(admitted.host.finalized_reply_ref().is_none());
        bootstrap_validation::settle_closed(&admitted)
            .await
            .unwrap();
    }
    runtime.shutdown().await.unwrap();
}
