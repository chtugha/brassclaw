//! Qualified Pair cases through actual private and operator HTTP ingress.
//! Native journal custody and settlement; no accepted model design or serving claim.
use super::*;
use crate::RebornRuntime;
use crate::{
    bootstrap_recipe_flow, bootstrap_validation,
    global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe},
    prefix_pair_case_capture::CapturedPairCases,
};
use async_trait::async_trait;
use brassclaw_engine::{
    executor::{
        retained_recipe::{RetainedProgram, RetainedRecipeExecution, RetainedToolPort},
        retained_source::InspectedRetainedProgram,
    },
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
    },
};
use brassclaw_monty_host::service::{PortFailure, TaskOutcome};
use brassclaw_skills::{
    prefix_pair_case_components::{PairCaseIds, pair_case_drafts},
    revision_store::PgComponentRevisionStore,
};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

struct Invocation {
    reference: String,
    source: Arc<CapturedPairCases>,
    inspected: Arc<InspectedRetainedProgram>,
    tools: Arc<dyn RetainedToolPort>,
    pool: Arc<brassclaw_pg::PgPool>,
    run: Uuid,
    preparation: crate::source_inspection::PreparedUtility,
}
#[async_trait]
impl InternalRecipeSource for Invocation {
    fn name(&self) -> &str {
        "development-pair-default-template-cases"
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: self.inspected.clone(),
            inputs: Some(self.source.input().into()),
            tools: Some(self.tools.clone()),
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        let bad = || PortFailure::new("pair_case_receipt_invalid").unwrap();
        if !std::ptr::eq(selected, self.inspected.as_ref()) {
            return Err(bad());
        }
        let mut client = self.pool.get().await.map_err(|_| bad())?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| bad())?;
        self.source
            .verify_receipt(
                &tx,
                self.run,
                selected,
                execution,
                receipt,
                &self.preparation,
            )
            .await
            .map_err(|_| bad())?;
        // Reject forged completion and unrelated run evidence using the same
        // actual completed execution; this does not mutate immutable journals.
        let mut forged: Value = serde_json::from_str(receipt).map_err(|_| bad())?;
        forged["provider_wire_qualified"] = json!(true);
        let forged = crate::prefix_generations::canonical(&forged).map_err(|_| bad())?;
        assert!(
            self.source
                .verify_receipt(
                    &tx,
                    self.run,
                    selected,
                    execution,
                    &forged,
                    &self.preparation
                )
                .await
                .is_err()
        );
        assert!(
            self.source
                .verify_receipt(
                    &tx,
                    Uuid::new_v4(),
                    selected,
                    execution,
                    receipt,
                    &self.preparation
                )
                .await
                .is_err()
        );
        let wrong = crate::source_inspection::PreparedUtility {
            worker: self.preparation.worker.clone(),
            limits: self.preparation.limits,
            revision: self.preparation.revision + 1,
        };
        assert!(
            self.source
                .verify_receipt(&tx, self.run, selected, execution, receipt, &wrong)
                .await
                .is_err()
        );
        tx.commit().await.map_err(|_| bad())?;
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pair_case_recipe_checks_source_handoffs_and_actual_durable_native_journal() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Arc::new(
        Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
            rig.build_input("pair-case-journal-owner", directory.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )))
        .await
        .unwrap(),
    );
    let catalogue = &runtime.installed_catalogue;
    let pool = catalogue.pool.clone();
    let fixture = crate::bootstrap_encoding_fixture::Fixture::retain(pool.clone())
        .await
        .unwrap();
    let template = crate::bootstrap_encoding_fixture::TEMPLATE;
    let sha = crate::bootstrap_encoding_fixture::checksum;
    let store = crate::prefix_model_designs::PgPrefixModelDesigns::new(
        pool.clone(),
        bootstrap_validation::NAMESPACE.into(),
    );
    let mut document = crate::prefix_pair_construction::tests::draft(
        &fixture.specification,
        &fixture.artifacts,
        "tokenizer_config.json",
        &sha(json!({"chat_template":template}).to_string().as_bytes()),
        json!("/chat_template"),
        &sha(template.as_bytes()),
        &sha(b"{}"),
    );
    document["sample_reference"] = json!("hello");
    document["construction"]["prefix"]["leading_literal"] = json!("");
    document["construction"]["prefix"]["trailing_literal"] = json!("");
    document["worked_cases"] = json!([
        {"id":"plain","mode":"plain","render_context_json":json!({"messages":[{"role":"user","content":"hello world"}]}).to_string(),"expected_fixed_prefix":"hello","formatted_prompt":" world","expected_request":"hello world","evidence":["template"]},
        {"id":"unicode","mode":"plain","render_context_json":json!({"messages":[{"role":"user","content":"hello ü\r\nworld"}]}).to_string(),"expected_fixed_prefix":"hello","formatted_prompt":" ü\r\nworld","expected_request":"hello ü\r\nworld","evidence":["template"]}
    ]);
    let RetainedProgram::Tools(existing) = catalogue._measurement_candidate.program() else {
        panic!("existing usages required")
    };
    let digest = brassclaw_skills::utf8_digest_components::usage_ids(
        existing.bindings()["0:5"].tool().uuid,
        crate::installed_monty_catalogue::id,
    );
    let encoding = brassclaw_skills::model_encoding_components::usage_ids(
        existing.bindings()["0:7"].tool().uuid,
        false,
        crate::installed_monty_catalogue::id,
    );
    let ids = PairCaseIds::allocate(crate::installed_monty_catalogue::id);
    let mut drafts = pair_case_drafts(ids, digest, encoding).unwrap();
    drafts.extend(
        brassclaw_skills::utf8_digest_components::utf8_digest_usage_drafts(digest).unwrap(),
    );
    drafts.extend(
        brassclaw_skills::model_encoding_components::model_encoding_usage_drafts(encoding).unwrap(),
    );
    for tool in [digest.tool, encoding.tool] {
        drafts.push(
            existing.inputs().instruction().snapshot().revisions()[&tool]
                .draft()
                .clone(),
        );
    }
    let revisions = PgComponentRevisionStore::new(pool.clone());
    let mut refs = Vec::new();
    for draft in &drafts {
        refs.push(revisions.retain_packaged_draft(draft).await.unwrap());
    }
    let all = revisions
        .read_exact(&[ids.recipe, digest.recipe, encoding.recipe], &refs)
        .await
        .unwrap();
    let program = Arc::new(
        prepare_retained_tool_program(
            compile_retained_recipe(
                Arc::new(all.select_roots(&[ids.recipe]).unwrap()),
                ids.recipe,
                "selected",
                WorkflowClass::Deterministic,
            )
            .unwrap(),
        )
        .unwrap(),
    );
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect_with_limits(
            RetainedProgram::Tools(program.clone()),
            &catalogue.worker,
            catalogue._measurement_candidate.preparation_limits(),
        )
        .await
        .unwrap(),
    );
    let root = crate::global_root_seed::retain_packaged_global_root(
        pool.clone(),
        crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
    )
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
    for case in ["matches", "changed-suffix", "missing-leading-prefix"] {
        let mut draft = document.clone();
        match case {
            "changed-suffix" => {
                draft["worked_cases"][0]["formatted_prompt"] = json!(" world wrong");
                draft["worked_cases"][0]["expected_request"] = json!("hello world wrong");
            }
            "missing-leading-prefix" => {
                draft["worked_cases"][0]["render_context_json"] =
                    json!(json!({"messages":[{"role":"user","content":"world hello"}]}).to_string())
            }
            _ => {}
        }
        let proposal = store
            .retain_pair_proposal(
                brassclaw_product_workflow::RetainPrefixDesignProposalRequest {
                    specification: fixture.specification.clone(),
                    proposal_bytes: draft.to_string(),
                },
            )
            .await
            .unwrap();
        let source = Arc::new(
            CapturedPairCases::capture(
                pool.clone(),
                bootstrap_validation::NAMESPACE,
                &fixture.specification,
                &proposal.id,
            )
            .await
            .unwrap(),
        );
        assert!(
            CapturedPairCases::capture(
                pool.clone(),
                "foreign-pair-cases",
                &fixture.specification,
                &proposal.id
            )
            .await
            .is_err()
        );
        let reference = format!("development-pair-cases:{case}:{}", Uuid::new_v4());
        let admitted =
            bootstrap_validation::admit(pool.clone(), "pair-case-journal-owner", &reference, None)
                .await
                .unwrap();
        let tools = crate::installed_monty_catalogue::pair_case_candidate_tools(
            pool.clone(),
            catalogue.kernel.clone(),
            admitted.host.clone(),
            admitted.admission.clone(),
            program.clone(),
        )
        .unwrap();
        let preparation = runtime
            .services
            .source_inspection
            .as_ref()
            .unwrap()
            .prepare()
            .await
            .unwrap();
        let observed = bootstrap_recipe_flow::execute(
            &admitted,
            bootstrap_recipe_flow::Request::contained(
                catalogue.worker.clone(),
                catalogue.startup_executor.clone(),
                &root,
                Arc::new(Invocation {
                    reference,
                    source,
                    inspected: inspected.clone(),
                    tools,
                    pool: pool.clone(),
                    run: admitted.attempt.run_id.as_uuid(),
                    preparation,
                }),
            )
            .unwrap(),
        )
        .await
        .unwrap();
        let success = case != "missing-leading-prefix";
        assert_eq!(
            matches!(
                observed.receipt.outcome,
                TaskOutcome::InternalCompleted { .. }
            ),
            success,
            "{case}: {:?}",
            observed.report
        );
        assert_eq!(observed.report["model_exchange_count"], 0);
        assert!(observed.report["reply"].is_null());
        assert_eq!(observed.report["semantic_approval"], false);
        if success {
            let result: Value =
                serde_json::from_str(observed.report["internal_receipt"].as_str().unwrap())
                    .unwrap();
            assert_eq!(result["all_cases_match"], case == "matches");
            assert_eq!(result["stable_prefix_encoding"], true);
            assert_eq!(result["cases"][0]["prefix_tokens"], 1);
            assert_eq!(result["cases"][1]["total_tokens"], 3);
        } else {
            observed
                .ports
                .observe_internal_execution(|execution| {
                    assert_eq!(execution.failed_step_id(), Some("0:5"));
                    assert!(!execution.observations().contains_key("0:6"));
                })
                .await
                .unwrap();
        }
        let count:i64=pool.get().await.unwrap().query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1 AND attempt_count=1",&[&admitted.attempt.run_id.as_uuid()]).await.unwrap().get(0);
        assert_eq!(count, 2);
        assert!(admitted.host.finalized_reply_ref().is_none());
        assert!(
            observed.exit.failure.is_none()
                && observed.exit.pending_admission.is_none()
                && observed.exit.queued_admissions.is_empty()
        );
        let exit = observed.exit.transport.unwrap();
        assert!(
            exit.exit_status.is_some()
                && exit.containment_error.is_none()
                && exit.reap_error.is_none()
                && exit.shutdown_failure.is_none()
        );
        bootstrap_validation::settle_closed(&admitted)
            .await
            .unwrap();
    }
    assert_admitted_pair_jobs(&runtime, &document).await;
    assert_eq!(
        pool.get()
            .await
            .unwrap()
            .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        approvals_before
    );
    shutdown_shared_runtime(runtime).await.unwrap();
    eprintln!(
        "pair-case candidate: complete real Recipe/native journal and operator HTTP jobs; exact source, receipt and preparation checks; wrong suffix is mismatch; missing leading prefix fails before comparison; no accepted model design"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pair_bootstrap_qualifies_exact_recipe_and_checks_retained_journal_without_replay() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("pair-bootstrap-owner", directory.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();
    let catalogue = &runtime.installed_catalogue;
    let qualification = catalogue
        .pair_case_qualification
        .as_ref()
        .expect("exact packaged pair workflow qualified before ingress");
    let program = qualification.program();
    let RetainedProgram::Tools(selected) = program.program() else {
        panic!("qualified pair Tool program required")
    };
    assert_eq!(selected.bindings().len(), 2);
    assert_eq!(
        selected
            .inputs()
            .instruction()
            .association_approvals()
            .len(),
        2
    );
    crate::bootstrap_pair_approval::assert_retained_proof_checks_journal(
        catalogue.pool.clone(),
        qualification,
        catalogue.kernel.as_ref(),
    )
    .await;
    let before: i64 = catalogue
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get(0);
    let again = crate::bootstrap_pair_approval::qualify(
        catalogue.pool.clone(),
        catalogue.kernel.clone(),
        &catalogue._pair_case_candidate,
        catalogue.compiler_approval.as_ref(),
        &catalogue.encoding_approvals,
        &catalogue.worker,
        catalogue.startup_executor.clone(),
    )
    .await
    .unwrap()
    .expect("retained proof reuse");
    assert_eq!(
        again
            .program()
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .unwrap()
            .exact_bytes(),
        program
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .unwrap()
            .exact_bytes()
    );
    assert_eq!(
        before,
        catalogue
            .pool
            .get()
            .await
            .unwrap()
            .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0)
    );
    runtime.shutdown().await.unwrap();
    eprintln!(
        "pair bootstrap: six exact cases/native journals, original template, captured limits, malformed proofs and incomplete intent rejected; retained read does not replay; no accepted model design or public job"
    );
}

async fn assert_admitted_pair_jobs(runtime: &Arc<RebornRuntime>, document: &Value) {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use brassclaw_product_workflow::{
        CapturePrefixModelRequest, PrefixModelTarget, RetainPrefixDesignProposalRequest,
        RetainPrefixModelArtifactsRequest, WebUiAuthenticatedCaller,
    };
    let pool = runtime.installed_catalogue.pool.clone();
    let namespace = runtime.webui_tenant_id();
    let repo = crate::PgProviderRepo::new(pool.as_ref().clone(), namespace);
    repo.initialize().await.unwrap();
    let definition = serde_json::from_value(json!({"id":"pair-admitted-data-model","aliases":[],
        "protocol":"open_ai_completions","default_base_url":"http://127.0.0.1:1/v1",
        "base_url_required":true,"api_key_required":false,"model_env":"UNUSED_ADMITTED_PAIR_MODEL",
        "default_model":"wordlevel-data-fixture","description":"Offline source data only"}))
    .unwrap();
    let provider = repo.replace_revision(definition, None).await.unwrap();
    let store =
        crate::prefix_model_designs::PgPrefixModelDesigns::new(pool.clone(), namespace.into());
    let specification = store
        .capture(
            CapturePrefixModelRequest {
                provider_id: "pair-admitted-data-model".into(),
                expected_provider_revision: provider.revision,
                model: "wordlevel-data-fixture".into(),
            },
            vec![PrefixModelTarget {
                provider_id: "pair-admitted-data-model".into(),
                provider_revision: provider.revision,
                model: "wordlevel-data-fixture".into(),
                consumer: "configured".into(),
                configured_context_window: None,
            }],
        )
        .await
        .unwrap();
    let model_card = "\u{feff}# Synthetic test model card ü\r\n{{ never_execute() }}\r\n<script>literal documentation</script>\r\n";
    let originals = [
        ("config.json", "{}".to_owned()),
        ("README.md", model_card.to_owned()),
        (
            "tokenizer.json",
            crate::bootstrap_encoding_fixture::tokenizer(),
        ),
        (
            "tokenizer_config.json",
            json!({"chat_template":crate::bootstrap_encoding_fixture::TEMPLATE}).to_string(),
        ),
    ];
    let files: serde_json::Map<String, Value> = originals
        .into_iter()
        .map(|(name, bytes)| {
            (
                name.into(),
                json!({"encoding":"base64","data":STANDARD.encode(bytes.as_bytes())}),
            )
        })
        .collect();
    let artifacts = crate::prefix_model_artifacts::PgPrefixModelArtifacts::new(
        pool.clone(),
        namespace.into(),
    )
    .retain_with_origin(
        RetainPrefixModelArtifactsRequest {
            specification: specification.id.clone(),
            artifact_set_bytes:
                json!({"format":"brassclaw-prefix-model-artifact-import/1","artifacts":files})
                    .to_string(),
        },
        crate::prefix_model_artifacts::ArtifactImportOrigin::Operator {
            actor: runtime.actor_user_id.to_string(),
        },
    )
    .await
    .unwrap();
    let caller = WebUiAuthenticatedCaller::new(
        runtime.thread_scope.tenant_id.clone(),
        runtime.actor_user_id.clone(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
    );
    let mut config = crate::WebuiServeConfig::new(
        runtime.thread_scope.tenant_id.clone(),
        Arc::new(PairOperator(runtime.actor_user_id.clone())),
        Vec::new(),
    )
    .with_default_agent_id(runtime.thread_scope.agent_id.clone());
    if let Some(project) = &runtime.thread_scope.project_id {
        config = config.with_default_project_id(project.clone());
    }
    let app = crate::webui_v2_app(
        crate::build_webui_services(runtime.clone(), None)
            .await
            .unwrap(),
        config,
    )
    .unwrap();
    let artifact_path = format!(
        "/api/webchat/v2/prefix-model-artifacts/{}/{}",
        specification.id, artifacts.id
    );
    let (read_status, detail) = pair_http(&app, "GET", &artifact_path, true).await;
    assert_eq!(read_status, axum::http::StatusCode::OK, "{detail}");
    let package: Value =
        serde_json::from_str(detail["artifact_set_bytes"].as_str().unwrap()).unwrap();
    assert_eq!(
        STANDARD
            .decode(package["artifacts"]["README.md"]["data"].as_str().unwrap())
            .unwrap(),
        model_card.as_bytes()
    );
    let observation: Value =
        serde_json::from_str(detail["format_observation"].as_str().unwrap()).unwrap();
    assert!(
        observation["templates"]
            .as_array()
            .unwrap()
            .iter()
            .all(|template| template["file"] != "README.md")
    );
    assert!(observation["effective_template"].is_null());
    // Exercise the actual rebuilt static bundle, not just an unembedded JS file.
    for path in [
        "js/pages/settings/components/prefix-pair-cases.js",
        "js/pages/settings/components/prefix-model-designs.js",
        "js/pages/settings/lib/settings-api.js",
        "js/pages/settings/lib/prefix-model-artifacts.js",
    ] {
        use tower::ServiceExt;
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/{path}"))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        assert_eq!(
            bytes.as_ref(),
            std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../brassclaw_webui_v2_static/static")
                    .join(path)
            )
            .unwrap()
        );
    }
    let root = runtime.global_monty_owner.client().root_identity();
    let mut original_pair: Option<(Uuid, String)> = None;
    for case in ["matches", "changed-suffix", "missing-leading-prefix"] {
        let mut draft = document.clone();
        draft["specification"] = json!(specification.id);
        draft["artifact_set"] = json!(artifacts.id);
        draft["evidence"].as_array_mut().unwrap().push(json!({"id":"model-card","file":"README.md",
            "file_sha256":crate::bootstrap_encoding_fixture::checksum(model_card.as_bytes()),"pointer":null}));
        draft["construction"]["memory"]["evidence"]
            .as_array_mut()
            .unwrap()
            .push(json!("model-card"));
        let mut invalid_document_pointer = draft.clone();
        invalid_document_pointer["evidence"][2]["pointer"] = json!("/not-a-json-document");
        assert!(
            store
                .retain_pair_proposal(RetainPrefixDesignProposalRequest {
                    specification: specification.id.clone(),
                    proposal_bytes: invalid_document_pointer.to_string(),
                })
                .await
                .is_err()
        );
        let mut invalid_document_template = draft.clone();
        invalid_document_template["template"]["evidence"] = json!("model-card");
        assert!(
            store
                .retain_pair_proposal(RetainPrefixDesignProposalRequest {
                    specification: specification.id.clone(),
                    proposal_bytes: invalid_document_template.to_string(),
                })
                .await
                .is_err()
        );

        match case {
            "changed-suffix" => {
                draft["worked_cases"][0]["formatted_prompt"] = json!(" world wrong");
                draft["worked_cases"][0]["expected_request"] = json!("hello world wrong");
            }
            "missing-leading-prefix" => {
                draft["worked_cases"][0]["render_context_json"] = json!(
                    json!({"messages":[{"role":"user","content":"world hello"}]}).to_string()
                );
            }
            _ => {}
        }
        let proposal = store
            .retain_pair_proposal(RetainPrefixDesignProposalRequest {
                specification: specification.id.clone(),
                proposal_bytes: draft.to_string(),
            })
            .await
            .unwrap();
        let id = Uuid::new_v4();
        eprintln!("private admitted Pair start: case={case} job={id}");
        let path = format!(
            "/api/webchat/v2/prefix-model-designs/proposals/{}/{}/jobs/{id}",
            specification.id, proposal.id
        );
        for method in ["GET", "POST"] {
            assert_eq!(
                pair_http(&app, method, &path, false).await.0,
                axum::http::StatusCode::UNAUTHORIZED
            );
        }
        assert!(
            crate::prefix_job_definitions::Store::new(pool.clone())
                .get(id)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            pair_http(&app, "GET", &path, true).await.0,
            axum::http::StatusCode::NOT_FOUND
        );
        let (accepted, detail) = pair_http(&app, "POST", &path, true).await;
        assert_eq!(accepted, axum::http::StatusCode::OK, "{detail}");
        assert_eq!(detail["target_kind"], "pair");
        assert_eq!(detail["specification"], specification.id);
        assert_eq!(detail["proposal"], proposal.id);
        assert!(detail.get("profile").is_none());
        assert!(detail.get("profile_revision").is_none());
        let retained = crate::prefix_job_definitions::Store::new(pool.clone())
            .get(id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retained.operation, "check_cases");
        assert!(retained.target.profile().is_none());
        let qualified = runtime
            .installed_catalogue
            .pair_case_qualification
            .as_ref()
            .unwrap()
            .program();
        assert_eq!(
            retained.program_bytes,
            qualified
                .program()
                .inputs()
                .instruction()
                .retained_selection()
                .unwrap()
                .exact_bytes()
        );
        let status = runtime
            .wait_for_terminal(
                &retained.scope,
                TurnRunId::from_uuid(id),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        let client = pool.get().await.unwrap();
        let calls = client.query("SELECT step_id,attempt_count,phase,answer_bytes FROM brassclaw_monty_tool_invocations WHERE run_id=$1 ORDER BY step_id", &[&id]).await.unwrap();
        assert_eq!(calls.len(), 2);
        for (row, step) in calls.iter().zip(["0:3", "0:5"]) {
            assert_eq!(row.get::<_, String>(0), step);
            assert_eq!(row.get::<_, i16>(1), 1);
            assert_eq!(row.get::<_, String>(2), "answered");
        }
        let row = client
            .query_one(
                "SELECT phase,outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
                &[&id],
            )
            .await
            .unwrap();
        let phase: String = row.get(0);
        let state = runtime
            .turn_coordinator
            .get_run_state(brassclaw_turns::GetRunStateRequest {
                scope: retained.scope.clone(),
                run_id: TurnRunId::from_uuid(id),
            })
            .await
            .unwrap();
        let native_kinds = calls
            .iter()
            .map(|row| {
                let bytes: Option<String> = row.get(3);
                let answer: brassclaw_monty_host::process::PortAnswer =
                    serde_json::from_str(bytes.as_deref().unwrap()).unwrap();
                match answer {
                    brassclaw_monty_host::process::PortAnswer::Return { .. } => "return".to_owned(),
                    brassclaw_monty_host::process::PortAnswer::TerminalError { reason_kind } => {
                        format!("terminal:{reason_kind}")
                    }
                    brassclaw_monty_host::process::PortAnswer::DomainError { reason_kind } => {
                        format!("domain:{reason_kind}")
                    }
                }
            })
            .collect::<Vec<_>>();
        eprintln!(
            "private Pair completion boundary: case={case} status={status:?} phase={phase} failure={:?} native={native_kinds:?}",
            state.failure.as_ref().map(|failure| failure.category())
        );
        let driver = runtime
            .monty_test_controls
            .0
            .test_settlement_observations(TurnRunId::from_uuid(id));
        let factory = runtime
            .monty_test_controls
            .1
            .test_settlement_observations(TurnRunId::from_uuid(id))
            .await;
        eprintln!("private Pair retained driver ({case}): {driver:?}");
        eprintln!("private Pair retained factory ({case}): {factory:?}");
        assert!(
            driver.is_ok() && factory.is_ok(),
            "actual retained settlement observation unavailable: {case}"
        );
        assert_eq!(phase, "settled", "case={case} status={status:?}");
        let outcome: Value = row.get::<_, Option<Value>>(1).unwrap();
        if case == "missing-leading-prefix" {
            assert_ne!(status, TurnStatus::Completed);
            assert_eq!(outcome["status"], "failed");
            assert!(outcome["internal_receipt"].is_null());
        } else {
            assert_eq!(status, TurnStatus::Completed);
            let result: Value =
                serde_json::from_str(outcome["internal_receipt"].as_str().unwrap()).unwrap();
            assert_eq!(result["all_cases_match"], case == "matches");
            assert_eq!(result["qualification"], "retained-template-cases-only");
            assert_eq!(result["provider_wire_qualified"], false);
            assert_eq!(result["proposal"], proposal.id);
            assert_eq!(result["specification"], specification.id);
        }
        assert!(
            runtime
                .read_latest_assistant_text(
                    &retained.thread_scope,
                    &retained.scope.thread_id,
                    TurnRunId::from_uuid(id)
                )
                .await
                .unwrap()
                .is_none()
        );
        assert!(runtime.inspect_prefix_job("base-prompt", id).await.is_err());
        let (read_status, projected) = pair_http(&app, "GET", &path, true).await;
        assert_eq!(read_status, axum::http::StatusCode::OK, "{projected}");
        assert_eq!(
            projected["run_status"],
            serde_json::to_value(status).unwrap()
        );
        assert_eq!(projected["admission_phase"], "settled");
        assert_eq!(projected["answered_tool_calls"], 2);
        assert_eq!(projected["pending_tool_calls"], 0);
        assert_eq!(projected["internal_receipt"], outcome["internal_receipt"]);
        let wrong_target = format!(
            "/api/webchat/v2/prefix-model-designs/proposals/{}/{}/jobs/{id}",
            specification.id,
            "f".repeat(64)
        );
        assert_eq!(
            pair_http(&app, "GET", &wrong_target, true).await.0,
            axum::http::StatusCode::NOT_FOUND
        );
        let profile_path = format!("/api/webchat/v2/prefixes/base-prompt/jobs/{id}");
        assert_eq!(
            pair_http(&app, "GET", &profile_path, true).await.0,
            axum::http::StatusCode::NOT_FOUND
        );

        let (reconnected, detail) = pair_http(&app, "POST", &path, true).await;
        assert_eq!(reconnected, axum::http::StatusCode::OK, "{detail}");
        assert_eq!(detail["admission_phase"], "settled");
        let reread = crate::prefix_job_definitions::Store::new(pool.clone())
            .get(id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reread.input, retained.input);
        assert_eq!(reread.program_bytes, retained.program_bytes);
        assert_eq!(
            client
                .query_one(
                    "SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
                    &[&id]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            2
        );
        if let Some((original_id, original_proposal)) = &original_pair {
            let conflicting_path = format!(
                "/api/webchat/v2/prefix-model-designs/proposals/{}/{}/jobs/{original_id}",
                specification.id, proposal.id
            );
            assert_eq!(
                pair_http(&app, "POST", &conflicting_path, true).await.0,
                axum::http::StatusCode::CONFLICT
            );
            assert!(
                runtime
                    .submit_prefix_pair_cases(
                        *original_id,
                        &specification.id,
                        &proposal.id,
                        caller.clone()
                    )
                    .await
                    .is_err()
            );
            runtime
                .submit_prefix_pair_cases(
                    *original_id,
                    &specification.id,
                    original_proposal,
                    caller.clone(),
                )
                .await
                .unwrap();
        } else {
            original_pair = Some((id, proposal.id.clone()));
        }
    }
    let invalid_id = Uuid::new_v4();
    for path in [
        format!("/api/webchat/v2/prefix-model-designs/proposals/bad/bad/jobs/{invalid_id}"),
        format!(
            "/api/webchat/v2/prefix-model-designs/proposals/{}/{}/jobs/not-a-uuid",
            specification.id,
            original_pair.as_ref().unwrap().1
        ),
    ] {
        for method in ["GET", "POST"] {
            assert_eq!(
                pair_http(&app, method, &path, true).await.0,
                axum::http::StatusCode::BAD_REQUEST
            );
        }
    }

    assert!(
        runtime
            .submit_prefix_pair_cases(invalid_id, "bad", "bad", caller.clone())
            .await
            .is_err()
    );
    assert!(
        crate::prefix_job_definitions::Store::new(pool.clone())
            .get(invalid_id)
            .await
            .unwrap()
            .is_none()
    );
    let foreign = WebUiAuthenticatedCaller::new(
        brassclaw_host_api::TenantId::new("foreign-pair-caller").unwrap(),
        caller.user_id.clone(),
        caller.agent_id.clone(),
        caller.project_id.clone(),
    );
    assert!(
        runtime
            .submit_prefix_pair_cases(
                invalid_id,
                &specification.id,
                &original_pair.as_ref().unwrap().1,
                foreign
            )
            .await
            .is_err()
    );
    assert!(
        crate::prefix_job_definitions::Store::new(pool.clone())
            .get(invalid_id)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(runtime.global_monty_owner.client().root_identity(), root);
    eprintln!(
        "operator admitted Pair jobs: actual authenticated HTTP submit/status and private immutable owner; qualified original program, complete owner packet, positive/mismatch/encoding-failure results, native journal custody, no reply/Base fallback, reconnect without replay and conflicting/foreign input rejection; no model design acceptance"
    );
}

struct PairOperator(brassclaw_host_api::UserId);
#[async_trait]
impl crate::WebuiAuthenticator for PairOperator {
    async fn authenticate(&self, token: &str) -> Option<brassclaw_host_api::UserId> {
        (token == "pair-operator").then(|| self.0.clone())
    }
    fn allows_operator_webui_config(&self) -> bool {
        true
    }
}

async fn pair_http(
    app: &axum::Router,
    method: &str,
    path: &str,
    authenticated: bool,
) -> (axum::http::StatusCode, Value) {
    use tower::ServiceExt;
    let mut request = axum::http::Request::builder().method(method).uri(path);
    if authenticated {
        request = request.header("Authorization", "Bearer pair-operator");
    }
    let response = app
        .clone()
        .oneshot(request.body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
