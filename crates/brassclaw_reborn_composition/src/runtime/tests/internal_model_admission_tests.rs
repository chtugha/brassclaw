//! Closed native admission/settlement acceptance, not workflow activation.
//! Real global owner, claims, selected usages, kernel, PG custody and SDK peer.
use super::*;
use crate::global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe};
use async_trait::async_trait;
use brassclaw_engine::{
    executor::{
        retained_recipe::{RetainedRecipeExecution, RetainedToolPort},
        retained_source::InspectedRetainedProgram,
    },
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::{RetainedToolProgram, prepare_retained_tool_program},
    },
};
use brassclaw_loop_support::{GovernorBackedAccountant, HostManagedModelGateway, ZeroCostTable};
use brassclaw_monty_host::{
    process::PortAnswer,
    service::{PortFailure, TaskInput},
};
use brassclaw_reborn::{
    driver_registry::{DriverRequirements, LoopDriverRegistryKey, RequirementLevel},
    loop_driver_host::{RebornLoopDriverHostFactory, TextOnlyLoopHostConfig},
    model_gateway::{LlmModelProfilePolicy, LlmProviderModelGateway},
    monty_model_capability::RecipeModelPrefix,
    monty_task_host::MontyTaskHost,
    turn_runner::{HostFactory, HostFactoryError},
};
use brassclaw_skills::revision_store::PgComponentRevisionStore;
use brassclaw_threads::{
    PgSessionThreadService, SessionThreadService, ThreadHistoryRequest, ThreadScope,
};
use brassclaw_turns::{
    InMemoryCheckpointStateStore, InMemoryLoopCheckpointStore,
    loop_exit::{LoopCompletionKind, LoopExit, LoopExitApplier},
    run_profile::{
        AgentLoopDriverError, AgentLoopDriverRunRequest, AgentLoopHostErrorKind,
        InMemoryLoopHostMilestoneSink, InstructionSafetyContext, LoopModelRequest, ModelProfileId,
        MontyTaskHandoff, MontyTurnDriverPort,
    },
    runner::{ClaimedTurnRun, RecordModelRouteSnapshotRequest, TurnRunTransitionPort},
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, OnceLock};
use uuid::Uuid;

fn rejected() -> PortFailure {
    PortFailure::new("internal_model_evidence_invalid").unwrap()
}
fn failed() -> AgentLoopDriverError {
    AgentLoopDriverError::Failed {
        reason_kind: "internal_model_selection_invalid".into(),
    }
}

#[tokio::test]
async fn native_internal_completion_upgrade_retains_legacy_origins_and_payloads() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let mut client = rig.pool.get().await.unwrap();
    let mut upgrade = client.transaction().await.unwrap();
    upgrade.batch_execute("CREATE SCHEMA admission_origin_upgrade; SET LOCAL search_path TO admission_origin_upgrade,public,pg_catalog;").await.unwrap();
    upgrade
        .batch_execute(include_str!(
            "../../../../brassclaw_pg/migrations/V099__monty_task_admission_receipts.sql"
        ))
        .await
        .unwrap();
    upgrade
        .batch_execute(include_str!(
            "../../../../brassclaw_pg/migrations/V122__monty_completed_turn_review_events.sql"
        ))
        .await
        .unwrap();
    let insert = "INSERT INTO brassclaw_monty_task_admissions(run_id,turn_id,scope,accepted_message_ref,runner_id,claim_checksum,admission_key,phase,trusted_internal_turn,started_at,settled_at,outcome) VALUES($1,$2,'{}','original:message',$3,$4,$5,'settled',$6,clock_timestamp(),clock_timestamp(),$7)";
    for trusted in [false, true] {
        let outcome = json!({"status":"failed","reason_kind":"original_outcome","original":"é\r\n🙂","null":null});
        upgrade
            .execute(
                insert,
                &[
                    &Uuid::new_v4(),
                    &Uuid::new_v4(),
                    &Uuid::new_v4(),
                    &&[0u8; 32][..],
                    &&[u8::from(trusted); 32][..],
                    &trusted,
                    &outcome,
                ],
            )
            .await
            .unwrap();
    }
    // Install the actual old monotonic/catalogue/model guards too. A generated
    // column test without these would miss BEFORE-trigger identity conflicts.
    for ddl in [
        include_str!(
            "../../../../brassclaw_pg/migrations/V107__protect_monty_admission_settlement.sql"
        ),
        include_str!(
            "../../../../brassclaw_pg/migrations/V174__monty_task_catalogue_reference.sql"
        ),
        include_str!("../../../../brassclaw_pg/migrations/V179__monty_task_model_selection.sql"),
    ] {
        upgrade.batch_execute(ddl).await.unwrap();
    }
    let before: Vec<Value> = upgrade.query("SELECT to_jsonb(a) FROM brassclaw_monty_task_admissions a ORDER BY trusted_internal_turn",&[]).await.unwrap().into_iter().map(|row|row.get(0)).collect();
    upgrade
        .batch_execute(include_str!(
            "../../../../brassclaw_pg/migrations/V191__monty_internal_completion_origin.sql"
        ))
        .await
        .unwrap();
    let after = upgrade.query("SELECT to_jsonb(a)-'source_binding_ref'-'result_only_completion',source_binding_ref,result_only_completion,trusted_internal_turn FROM brassclaw_monty_task_admissions a ORDER BY trusted_internal_turn",&[]).await.unwrap();
    for (old, row) in before.iter().zip(after) {
        assert_eq!(
            row.get::<_, Value>(0),
            *old,
            "upgrade must not rewrite historical input/outcome/evidence"
        );
        assert!(
            row.get::<_, Option<String>>(1).is_none(),
            "unknown original source stays unknown"
        );
        assert_eq!(row.get::<_, bool>(2), row.get::<_, bool>(3));
    }
    let insert = "INSERT INTO brassclaw_monty_task_admissions(run_id,turn_id,scope,accepted_message_ref,runner_id,claim_checksum,admission_key,phase,trusted_internal_turn,source_binding_ref) VALUES($1,$2,'{}','original:message',$3,$4,$5,'reserved',$6,$7) RETURNING result_only_completion";
    for (n, source, trusted, result_only) in [
        (
            2u8,
            brassclaw_turns::run_profile::INTERNAL_MODEL_SOURCE_BINDING,
            false,
            true,
        ),
        (
            3,
            brassclaw_turns::run_profile::TRUSTED_INTERNAL_SOURCE_BINDING,
            true,
            true,
        ),
        (4, "ordinary-ingress", false, false),
    ] {
        let run = Uuid::new_v4();
        let row = upgrade
            .query_one(
                insert,
                &[
                    &run,
                    &Uuid::new_v4(),
                    &Uuid::new_v4(),
                    &&[0u8; 32][..],
                    &&[n; 32][..],
                    &trusted,
                    &source,
                ],
            )
            .await
            .unwrap();
        assert_eq!(row.get::<_, bool>(0), result_only);
        assert_eq!(upgrade.execute("UPDATE brassclaw_monty_task_admissions SET phase='started',started_at=clock_timestamp() WHERE run_id=$1",&[&run]).await.unwrap(),1);
        assert_eq!(
            upgrade
                .execute(
                    "UPDATE brassclaw_monty_task_admissions SET phase=phase WHERE run_id=$1",
                    &[&run]
                )
                .await
                .unwrap(),
            1
        );
        assert_eq!(upgrade.execute("UPDATE brassclaw_monty_task_admissions SET phase='settled',settled_at=clock_timestamp(),outcome='{\"status\":\"failed\",\"reason_kind\":\"original_closed\"}' WHERE run_id=$1",&[&run]).await.unwrap(),1);
    }
    for (source, trusted) in [
        ("", false),
        (
            brassclaw_turns::run_profile::INTERNAL_MODEL_SOURCE_BINDING,
            true,
        ),
        (
            brassclaw_turns::run_profile::TRUSTED_INTERNAL_SOURCE_BINDING,
            false,
        ),
        ("ordinary-ingress", true),
    ] {
        let probe = upgrade.transaction().await.unwrap();
        let error = probe
            .query_one(
                insert,
                &[
                    &Uuid::new_v4(),
                    &Uuid::new_v4(),
                    &Uuid::new_v4(),
                    &&[0u8; 32][..],
                    &&[9u8; 32][..],
                    &trusted,
                    &source,
                ],
            )
            .await
            .unwrap_err();
        assert_eq!(error.code().unwrap().code(), "23514");
        probe.rollback().await.unwrap();
    }
    for statement in [
        "UPDATE brassclaw_monty_task_admissions SET source_binding_ref='new-source'",
        "UPDATE brassclaw_monty_task_admissions SET trusted_internal_turn=NOT trusted_internal_turn",
    ] {
        let probe = upgrade.transaction().await.unwrap();
        assert_eq!(
            probe
                .batch_execute(statement)
                .await
                .unwrap_err()
                .code()
                .unwrap()
                .code(),
            "23514"
        );
        probe.rollback().await.unwrap();
    }
    upgrade.rollback().await.unwrap();
}

#[tokio::test]
async fn native_internal_completion_requires_original_claim_source_and_rejects_reply_settlement() {
    use crate::pg_monty_admission::PgMontyAdmission;
    use brassclaw_turns::{SourceBindingRef, run_profile::MontyTaskAttempt};
    let rig = crate::runtime::test_pg::pg_rig().await;
    let (_, context, _, claim) = crate::native_admission_fixture::claim_with_source(
        rig.pool.clone(),
        "private source",
        brassclaw_turns::run_profile::INTERNAL_MODEL_SOURCE_BINDING,
    )
    .await;
    let attempt = MontyTaskAttempt {
        run_id: claim.state.run_id,
        runner_id: claim.runner_id,
        lease_token: claim.lease_token,
    };
    for source in [
        None,
        Some(SourceBindingRef::new("ordinary-ingress").unwrap()),
    ] {
        let reservation =
            PgMontyAdmission::prepare(rig.pool.clone(), &context, attempt, source.as_ref())
                .unwrap();
        assert!(
            matches!(reservation.persist_reservation().await,Err(AgentLoopDriverError::Failed{reason_kind}) if reason_kind=="monty_admission_fenced")
        );
    }
    assert!(
        PgMontyAdmission::prepare(
            rig.pool.clone(),
            &context.clone().with_trusted_internal_turn(),
            attempt,
            Some(&claim.state.source_binding_ref)
        )
        .is_err()
    );
    let reservation = PgMontyAdmission::prepare(
        rig.pool.clone(),
        &context,
        attempt,
        Some(&claim.state.source_binding_ref),
    )
    .unwrap();
    reservation.persist_reservation().await.unwrap();
    reservation.check_and_start().await.unwrap();
    assert!(
        matches!(reservation.settle(json!({"status":"completed","reply_ref":"message:forbidden"})).await,Err(AgentLoopDriverError::Failed{reason_kind}) if reason_kind=="monty_admission_outcome_invalid")
    );
    let row = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT phase,outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&context.run_id.as_uuid()],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "started");
    assert!(row.get::<_, Option<Value>>(1).is_none());
    reservation
        .settle(json!({"status":"failed","reason_kind":"bootstrap_validation_closed"}))
        .await
        .unwrap();
    let (_, ordinary, _, ordinary_claim) = crate::native_admission_fixture::claim_with_source(
        rig.pool.clone(),
        "ordinary source",
        "ordinary-ingress",
    )
    .await;
    let forged = PgMontyAdmission::prepare(
        rig.pool.clone(),
        &ordinary,
        MontyTaskAttempt {
            run_id: ordinary_claim.state.run_id,
            runner_id: ordinary_claim.runner_id,
            lease_token: ordinary_claim.lease_token,
        },
        Some(&claim.state.source_binding_ref),
    )
    .unwrap();
    assert!(
        matches!(forged.persist_reservation().await,Err(AgentLoopDriverError::Failed{reason_kind}) if reason_kind=="monty_admission_fenced")
    );
    assert_eq!(
        rig.pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_task_admissions WHERE run_id=$1",
                &[&ordinary.run_id.as_uuid()]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

// Scope-dependent construction delegates to the actual Reborn HostFactory.
// This closed profile needs model/prompt/transcript ports; native selected Tool
// bindings are supplied separately by the real kernel, with no provider Tools.
struct ReviewHostFactory {
    pool: Arc<brassclaw_pg::PgPool>,
    gateway: Arc<dyn HostManagedModelGateway>,
    governor: Arc<dyn brassclaw_resources::ResourceGovernor>,
}
#[async_trait]
impl HostFactory for ReviewHostFactory {
    async fn create_host(
        &self,
        claimed: &ClaimedTurnRun,
    ) -> Result<
        Box<dyn brassclaw_turns::run_profile::AgentLoopDriverHost + Send + Sync>,
        HostFactoryError,
    > {
        let scope = ThreadScope {
            tenant_id: claimed.state.scope.tenant_id.clone(),
            agent_id: claimed.state.scope.agent_id.clone().unwrap(),
            project_id: claimed.state.scope.project_id.clone(),
            owner_user_id: claimed.state.scope.explicit_owner_user_id().cloned(),
        };
        let account = brassclaw_resources::ResourceAccount::user(
            scope.tenant_id.clone(),
            claimed.state.actor.as_ref().unwrap().user_id.clone(),
        );
        self.governor
            .set_limit(
                account,
                brassclaw_resources::ResourceLimits {
                    period: brassclaw_resources::BudgetPeriod::Rolling24h,
                    ..Default::default()
                },
            )
            .unwrap();
        let mut requirements = DriverRequirements::all_optional();
        requirements.model = RequirementLevel::Required;
        requirements.prompt = RequirementLevel::Required;
        requirements.transcript = RequirementLevel::Required;
        let key = LoopDriverRegistryKey::from_descriptor(&claimed.resolved_run_profile.loop_driver)
            .unwrap();
        let route = brassclaw_reborn::model_routes::ModelRoute::new(
            "internal-result-provider",
            "internal-result-model",
        )
        .unwrap();
        let resolver = Arc::new(
            brassclaw_reborn::model_routes::StaticModelRouteResolver::new(
                brassclaw_reborn::model_routes::ModelRoutePolicy::new(
                    brassclaw_reborn::model_routes::ModelSelectionMode::ManagedOnly,
                )
                .with_approved_route(route.clone()),
            )
            .with_route(brassclaw_reborn::model_routes::ModelSlot::Default, route),
        );
        let factory = RebornLoopDriverHostFactory::new(
            Arc::new(PgSessionThreadService::new(
                self.pool.clone(),
                scope.tenant_id.as_str(),
            )),
            scope,
            self.gateway.clone(),
            Arc::new(InMemoryCheckpointStateStore::default()),
            Arc::new(brassclaw_turns::PgTurnStateStore::new(
                self.pool.clone(),
                claimed.state.scope.tenant_id.as_str(),
            )),
            Arc::new(InMemoryLoopCheckpointStore::default()),
            Arc::new(InMemoryLoopHostMilestoneSink::default()),
            TextOnlyLoopHostConfig {
                max_messages: 8,
                require_model_route_snapshot: false,
            },
            InstructionSafetyContext::local_development_noop(),
        )
        .with_model_route_resolver(resolver)
        .with_driver_requirements(std::collections::HashMap::from([(key, requirements)]))
        .with_model_budget_accountant(Arc::new(GovernorBackedAccountant::new(
            self.governor.clone(),
            Arc::new(ZeroCostTable),
        )))
        .with_token_budget_mode(|| false);
        factory.create_host(claimed).await
    }
}

struct ModelSource {
    reference: String,
    inspected: Arc<InspectedRetainedProgram>,
    program: Arc<RetainedToolProgram>,
    prompt: Value,
    host: Arc<MontyTaskHost>,
    admission: Arc<crate::pg_monty_admission::PgMontyAdmission>,
    pool: Arc<brassclaw_pg::PgPool>,
    kernel: Arc<dyn crate::monty_kernel::MontyKernelSnapshot>,
    image: [u8; 32],
    tools: OnceLock<Arc<dyn RetainedToolPort>>,
}
#[async_trait]
impl InternalRecipeSource for ModelSource {
    fn name(&self) -> &str {
        "closed-native-model-result"
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    fn captured_program(&self) -> Option<Arc<InspectedRetainedProgram>> {
        Some(self.inspected.clone())
    }
    fn bind_native_runtimes(&self) -> Result<(), PortFailure> {
        if !self.admission.task_models_captured() {
            return Err(rejected());
        }
        let reference = "closed-native-model-prefix".to_owned();
        let prefix = RecipeModelPrefix::new(
            reference.clone(),
            Arc::from("Closed interface fixture; return the requested data."),
        )
        .map_err(|_| rejected())?;
        let responses = self.admission.model_response_store(reference);
        let tools = crate::installed_monty_catalogue::model_recipe_candidate_tools(
            self.pool.clone(),
            self.kernel.clone(),
            self.host.clone(),
            self.admission.clone(),
            self.program.clone(),
            (prefix, responses),
            self.image,
        )
        .map_err(|_| rejected())?;
        self.tools.set(tools).map_err(|_| rejected())
    }
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: self.inspected.clone(),
            inputs: Some(json!({"prompt":self.prompt}).into()),
            tools: Some(self.tools.get().ok_or_else(rejected)?.clone()),
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        if !std::ptr::eq(selected, self.inspected.as_ref())
            || !execution.is_complete()
            || execution.transport_failure().is_some()
            || self.host.finalized_reply_ref().is_some()
            || execution.host_answers().len() != self.program.bindings().len()
        {
            return Err(rejected());
        }
        let Some((last_step, PortAnswer::Return { value })) = execution.host_answers().last()
        else {
            return Err(rejected());
        };
        if value.as_value().as_str() != Some(receipt) {
            return Err(rejected());
        }
        let rows = self.pool.get().await.map_err(|_| rejected())?.query(
            "SELECT i.step_id,i.attempt_count::bigint,i.arguments_bytes,r.response_bytes FROM brassclaw_monty_tool_invocations i JOIN brassclaw_monty_model_responses r USING(invocation_id) WHERE i.run_id=$1",
            &[&self.host.run_context().expect("actual native chat fixture").run_id.as_uuid()],
        ).await.map_err(|_| rejected())?;
        if rows.len() != self.program.bindings().len() {
            return Err(rejected());
        }
        for row in rows {
            if row.get::<_, i64>(1) != 1
                || serde_json::from_str::<Value>(row.get::<_, &str>(2)).map_err(|_| rejected())?
                    != json!({"prompt":self.prompt})
            {
                return Err(rejected());
            }
            if row.get::<_, &str>(0) == last_step
                && serde_json::from_str::<Value>(row.get::<_, &str>(3)).map_err(|_| rejected())?
                    != serde_json::from_str::<Value>(receipt).map_err(|_| rejected())?
            {
                return Err(rejected());
            }
        }
        Ok(())
    }
}
struct ModelCatalogue {
    pool: Arc<brassclaw_pg::PgPool>,
    inspected: Arc<InspectedRetainedProgram>,
    program: Arc<RetainedToolProgram>,
    kernel: Arc<dyn crate::monty_kernel::MontyKernelSnapshot>,
    image: [u8; 32],
    prompt: Value,
    last_host: Mutex<Option<Arc<MontyTaskHost>>>,
}
#[async_trait]
impl crate::global_task_factory::MontyCatalogueProvider for ModelCatalogue {
    async fn capture_internal(
        &self,
        host: Arc<MontyTaskHost>,
        admission: Arc<crate::pg_monty_admission::PgMontyAdmission>,
        input: &TaskInput,
    ) -> Result<Option<Arc<dyn InternalRecipeSource>>, AgentLoopDriverError> {
        if !host.is_result_only_completion()
            || host
                .run_context()
                .expect("actual native chat fixture")
                .trusted_internal_turn
        {
            return Err(failed());
        }
        *self.last_host.lock().unwrap() = Some(host.clone());
        Ok(Some(Arc::new(ModelSource {
            reference: input.user_input.clone(),
            inspected: self.inspected.clone(),
            program: self.program.clone(),
            prompt: self.prompt.clone(),
            host,
            admission,
            pool: self.pool.clone(),
            kernel: self.kernel.clone(),
            image: self.image,
            tools: OnceLock::new(),
        })))
    }
    async fn prepare(
        &self,
        _host: Arc<MontyTaskHost>,
        _admission: Arc<crate::pg_monty_admission::PgMontyAdmission>,
        _input: &TaskInput,
    ) -> Result<Arc<dyn crate::global_task_factory::CapturedMontyCatalogue>, AgentLoopDriverError>
    {
        Err(failed())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_internal_model_admission_settles_result_only_without_deterministic_bypass_or_reply()
{
    use axum::{Json, extract::State, routing::post};
    let calls: Arc<tokio::sync::Mutex<Vec<Value>>> = Default::default();
    async fn complete(
        State(calls): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        calls.lock().await.push(body.clone());
        Json(
            json!({"id":"closed-internal-native","object":"chat.completion","created":1,"model":body["model"],
            "choices":[{"index":0,"message":{"role":"assistant","content":"Private native result"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":8,"completion_tokens":4,"total_tokens":12}}),
        )
    }
    let (address, stop, server) = crate::test_loopback_http::spawn(
        axum::Router::new()
            .route("/v1/chat/completions", post(complete))
            .with_state(calls.clone()),
    )
    .await;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Arc::new(
        Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
            rig.build_input("internal-model-owner", directory.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )))
        .await
        .unwrap(),
    );
    let catalogue = &runtime.installed_catalogue;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let original = crate::native_model_usage_tests::program_with_model_input(&store, None).await;
    let tool = original.bindings().values().next().unwrap().tool();
    let draft = brassclaw_skills::model_completion_components::model_completion_tool_draft(
        tool.uuid,
        &hex::encode(catalogue._image.checksum()),
    )
    .unwrap();
    let successor = store.stage(&draft, tool.version).await.unwrap();
    rig.pool
        .get()
        .await
        .unwrap()
        .execute(
            "INSERT INTO brassclaw_instance_tool_settings(tool_id) VALUES($1)",
            &[&tool.uuid],
        )
        .await
        .unwrap();
    let mut refs: Vec<_> = original
        .inputs()
        .instruction()
        .snapshot()
        .revisions()
        .values()
        .filter(|r| r.reference().uuid != tool.uuid)
        .map(|r| r.reference())
        .collect();
    refs.push(successor);
    let root = original.inputs().instruction().recipe().uuid;
    let snapshot = Arc::new(
        store
            .read_exact(
                &[root],
                &refs,
                brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
            )
            .await
            .unwrap(),
    );
    let program = Arc::new(
        prepare_retained_tool_program(
            compile_retained_recipe(snapshot, root, "selected", WorkflowClass::RequiresModel)
                .unwrap(),
        )
        .unwrap(),
    );
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect(
            brassclaw_engine::executor::retained_recipe::RetainedProgram::Tools(program.clone()),
            &catalogue.worker,
        )
        .await
        .unwrap(),
    );
    let model = "internal-result-model";
    let config = brassclaw_llm::build_llm_config_with_provider_policy(
        brassclaw_llm::ResolvedProviderConfig::Registry(
            brassclaw_llm::RegistryProviderConfig::generic(
                brassclaw_llm::ProviderProtocol::OpenAiCompletions,
                "internal-result-provider",
                None,
                format!("http://{address}/v1"),
                model,
            ),
        ),
        brassclaw_llm::ProviderRequestPolicy {
            max_retries: 0,
            ..Default::default()
        },
        Default::default(),
    )
    .unwrap();
    let provider = brassclaw_llm::build_static_provider_chain(
        &config,
        Arc::new(brassclaw_llm::SessionManager::new(Default::default())),
    )
    .await
    .unwrap();
    let (_, context, state, mut claimed) = crate::native_admission_fixture::claim_with_source(
        rig.pool.clone(),
        "closed model result",
        brassclaw_turns::run_profile::INTERNAL_MODEL_SOURCE_BINDING,
    )
    .await;
    let route = brassclaw_turns::run_profile::LoopModelRouteSnapshot::try_new(
        "internal-result-provider",
        model,
        "fixture:internal-model-v1",
        "auth:none",
    )
    .unwrap();
    claimed.state = state
        .record_model_route_snapshot(RecordModelRouteSnapshotRequest {
            run_id: context.run_id,
            runner_id: claimed.runner_id,
            lease_token: claimed.lease_token,
            snapshot: route,
        })
        .await
        .unwrap();
    let gateway = Arc::new(LlmProviderModelGateway::with_provider_identity(
        "internal-result-provider",
        provider,
        LlmModelProfilePolicy::new()
            .allow_model_profile(
                context.resolved_run_profile.model_profile_id.clone(),
                Some(model.into()),
            )
            .allow_model_profile(
                ModelProfileId::new("interactive_model").unwrap(),
                Some(model.into()),
            ),
    ));
    let host_factory = ReviewHostFactory {
        pool: rig.pool.clone(),
        gateway,
        governor: runtime.resource_governor.clone(),
    };
    brassclaw_reborn::loop_driver_host::init_compaction_summarizer(
        include_str!("../../../../brassclaw_loop_support/prompts/compaction_summarizer_fresh.md")
            .into(),
    );
    let ports_host: Arc<dyn brassclaw_turns::run_profile::AgentLoopDriverHost + Send + Sync> =
        Arc::from(host_factory.create_host(&claimed).await.unwrap());
    let handoff = MontyTaskHandoff::new_claimed(
        AgentLoopDriverRunRequest {
            turn_id: context.turn_id,
            run_id: context.run_id,
            resolved_run_profile: claimed.resolved_run_profile.clone(),
        },
        Arc::new(claimed.clone()),
        ports_host,
    )
    .unwrap();
    let selected = Arc::new(ModelCatalogue {
        pool: rig.pool.clone(),
        inspected,
        program: program.clone(),
        kernel: catalogue.kernel.clone(),
        image: catalogue._image.checksum(),
        prompt: json!({"user_query":"private data ü🙂","chat_history":[],"max_output_tokens":37}),
        last_host: Mutex::new(None),
    });
    let service = runtime.global_monty_owner.client();
    let identity = service.root_identity();
    let factory = Arc::new(
        crate::global_task_factory::OwnedGlobalTaskFactory::new(
            rig.pool.clone(),
            runtime.global_monty_owner.ownership_check(),
            selected.clone(),
            service.clone(),
            runtime
                .monty_settings_owner
                .store()
                .recipe_capacity_source(),
            runtime
                .monty_settings_owner
                .store()
                .cancellation_policy_source(),
            None,
        )
        .unwrap(),
    );
    let thread_scope = ThreadScope {
        tenant_id: context.scope.tenant_id.clone(),
        agent_id: context.scope.agent_id.clone().unwrap(),
        project_id: context.scope.project_id.clone(),
        owner_user_id: None,
    };
    let threads = Arc::new(PgSessionThreadService::new(
        rig.pool.clone(),
        context.scope.tenant_id.as_str(),
    ));
    let driver = crate::global_monty_driver::GlobalMontyDriver::new(
        service.clone(),
        threads.clone(),
        factory,
    )
    .unwrap();
    let exit = driver.drive_turn(handoff).await.unwrap();
    let LoopExit::Completed(completed) = &exit else {
        panic!("actual internal completion required: {exit:?}")
    };
    assert_eq!(completed.completion_kind, LoopCompletionKind::ResultOnly);
    assert!(completed.reply_message_refs.is_empty());
    assert_eq!(completed.result_refs.len(), 1);
    let host = selected.last_host.lock().unwrap().clone().unwrap();
    assert!(host.is_result_only_completion());
    assert!(
        !host
            .run_context()
            .expect("actual native chat fixture")
            .trusted_internal_turn,
        "model work must not be relabeled deterministic"
    );
    let attempted_reply = host
        .finalize_assistant_message(brassclaw_turns::run_profile::FinalizeAssistantMessage {
            reply: brassclaw_turns::run_profile::AssistantReply {
                content: "must not publish".into(),
            },
        })
        .await
        .unwrap_err();
    assert_eq!(attempted_reply.kind, AgentLoopHostErrorKind::PolicyDenied);
    let row=rig.pool.get().await.unwrap().query_one("SELECT source_binding_ref,trusted_internal_turn,result_only_completion,phase,outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",&[&context.run_id.as_uuid()]).await.unwrap();
    assert_eq!(
        row.get::<_, String>(0),
        brassclaw_turns::run_profile::INTERNAL_MODEL_SOURCE_BINDING
    );
    assert!(!row.get::<_, bool>(1));
    assert!(row.get::<_, bool>(2));
    assert_eq!(row.get::<_, String>(3), "settled");
    let outcome: Value = row.get(4);
    assert!(outcome["reply_ref"].is_null());
    assert_eq!(outcome["execution"]["intent_outcome"], "internal_recipe");
    let sent = calls.lock().await;
    assert_eq!(sent.len(), program.bindings().len());
    for body in &*sent {
        assert_eq!(body["model"], model);
        assert_eq!(
            body.get("max_completion_tokens")
                .or_else(|| body.get("max_tokens")),
            Some(&json!(37))
        );
    }
    drop(sent);
    let fallback=Arc::new(brassclaw_reborn::loop_exit_applier::ThreadCheckpointLoopExitEvidencePort::new_with_thread_scope(threads.clone(),state.clone(),Arc::new(InMemoryLoopCheckpointStore::default()),thread_scope.clone()));
    let evidence = Arc::new(crate::monty_result_evidence::MontyResultEvidence::new(
        rig.pool.clone(),
        fallback,
    ));
    let applier = LoopExitApplier::new(state.clone(), evidence);
    assert_eq!(
        applier.apply(&claimed, exit).await.unwrap().status,
        brassclaw_turns::TurnStatus::Completed
    );
    let history = threads
        .list_thread_history(ThreadHistoryRequest {
            scope: thread_scope,
            thread_id: context.thread_id.clone(),
        })
        .await
        .unwrap();
    assert!(
        history
            .messages
            .iter()
            .all(|message| message.kind != brassclaw_threads::MessageKind::Assistant)
    );
    assert_eq!(service.root_identity(), identity);
    let reviews = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM brassclaw_monty_review_events WHERE run_id=$1",
            &[&context.run_id.as_uuid()],
        )
        .await
        .unwrap();
    assert_eq!(
        reviews.get::<_, i64>(0),
        0,
        "private model completion must not create No-Match learning work"
    );
    let unchanged=rig.pool.get().await.unwrap().execute("UPDATE brassclaw_monty_task_admissions SET source_binding_ref='changed-origin' WHERE run_id=$1",&[&context.run_id.as_uuid()]).await.unwrap_err();
    assert_eq!(unchanged.code().unwrap().code(), "23514");
    assert_eq!(calls.lock().await.len(), program.bindings().len());
    // The same shared bootstrap admission accepts an actual existing host factory
    // for a closed first review, without pretending it is already qualified.
    let admitted = crate::bootstrap_validation::admit(
        rig.pool.clone(),
        "model-host-review",
        "closed review source",
        Some(&host_factory),
    )
    .await
    .unwrap();
    assert!(admitted.host.is_result_only_completion());
    assert!(
        !admitted
            .host
            .run_context()
            .expect("actual native chat fixture")
            .trusted_internal_turn
    );
    assert!(!admitted.admission.task_models_captured());
    let foreign = crate::bootstrap_validation::admit(
        rig.pool.clone(),
        "other-model-host-review",
        "different original review",
        Some(&host_factory),
    )
    .await
    .unwrap();
    assert!(
        admitted
            .admission
            .check_model_preparation(&foreign.host)
            .await
            .is_err()
    );
    assert!(!admitted.admission.task_models_captured());
    foreign.admission.check_and_start().await.unwrap();
    crate::bootstrap_validation::settle_closed(&foreign)
        .await
        .unwrap();
    let root_definition = crate::global_root_seed::retain_packaged_global_root(
        rig.pool.clone(),
        crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
    )
    .await
    .unwrap();
    let source = Arc::new(ModelSource {
        reference: "closed review source".into(),
        inspected: selected.inspected.clone(),
        program: program.clone(),
        prompt: selected.prompt.clone(),
        host: admitted.host.clone(),
        admission: admitted.admission.clone(),
        pool: rig.pool.clone(),
        kernel: catalogue.kernel.clone(),
        image: catalogue._image.checksum(),
        tools: OnceLock::new(),
    });
    // This is the existing contained development observer, not live admission
    // or a second production VM. Its model branch must use the same original
    // capture owner before binding runtimes or starting its first operation.
    let observed = crate::bootstrap_recipe_flow::execute(
        &admitted,
        crate::bootstrap_recipe_flow::Request::contained(
            catalogue.worker.clone(),
            catalogue.startup_executor.clone(),
            &root_definition,
            source.clone(),
        )
        .unwrap(),
    )
    .await
    .unwrap();
    assert!(admitted.admission.task_models_captured());
    assert!(source.tools.get().is_some());
    assert!(matches!(
        observed.receipt.outcome,
        brassclaw_monty_host::service::TaskOutcome::InternalCompleted { .. }
    ));
    assert!(observed.report["reply"].is_null());
    assert_eq!(observed.report["semantic_approval"], false);
    assert!(observed.exit.failure.is_none());
    let transport = observed.exit.transport.as_ref().unwrap();
    assert!(
        transport.exit_status.is_some()
            && transport.containment_error.is_none()
            && transport.reap_error.is_none()
    );
    assert_eq!(
        runtime.global_monty_owner.client().root_identity(),
        identity
    );
    assert!(
        admitted
            .admission
            .check_model_preparation(&admitted.host)
            .await
            .is_err(),
        "started work cannot reenter pre-effect preparation"
    );
    assert_eq!(calls.lock().await.len(), 2 * program.bindings().len());
    crate::bootstrap_validation::settle_closed(&admitted)
        .await
        .unwrap();
    // Exercise real host validation failure, not a substitute factory returning
    // a manufactured error. No admission/fallback/model effect may result.
    let mut wrong_scope = admitted
        .host
        .run_context()
        .expect("actual native chat fixture")
        .scope
        .clone();
    wrong_scope.agent_id = Some(brassclaw_host_api::AgentId::new("wrong-review-agent").unwrap());
    let wrong_thread_scope = ThreadScope {
        tenant_id: wrong_scope.tenant_id.clone(),
        agent_id: wrong_scope.agent_id.clone().unwrap(),
        project_id: wrong_scope.project_id.clone(),
        owner_user_id: wrong_scope.explicit_owner_user_id().cloned(),
    };
    let bad_factory = RebornLoopDriverHostFactory::new(
        Arc::new(PgSessionThreadService::new(
            rig.pool.clone(),
            wrong_scope.tenant_id.as_str(),
        )),
        wrong_thread_scope,
        host_factory.gateway.clone(),
        Arc::new(InMemoryCheckpointStateStore::default()),
        admitted.state.clone(),
        Arc::new(InMemoryLoopCheckpointStore::default()),
        Arc::new(InMemoryLoopHostMilestoneSink::default()),
        TextOnlyLoopHostConfig {
            max_messages: 8,
            require_model_route_snapshot: false,
        },
        InstructionSafetyContext::local_development_noop(),
    );
    assert!(
        crate::bootstrap_validation::admit(
            rig.pool.clone(),
            "wrong-model-host",
            "must not dispatch",
            Some(&bad_factory)
        )
        .await
        .is_err()
    );
    let snapshots = rig
        .pool
        .get()
        .await
        .unwrap()
        .query(
            "SELECT payload FROM brassclaw_turns WHERE tenant_id=$1 AND status='snapshot'",
            &[&crate::bootstrap_validation::NAMESPACE],
        )
        .await
        .unwrap();
    let failed_runs: Vec<_> = snapshots
        .into_iter()
        .flat_map(|row| {
            serde_json::from_value::<brassclaw_turns::TurnPersistenceSnapshot>(row.get(0))
                .unwrap()
                .runs
        })
        .filter(|run| run.status == brassclaw_turns::TurnStatus::Failed)
        .collect();
    assert_eq!(
        failed_runs.len(),
        1,
        "the original effect-free preparation failure must close its real run"
    );
    assert_eq!(
        rig.pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_task_admissions WHERE run_id=$1",
                &[&failed_runs[0].run_id.as_uuid()]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    // Deterministic ingress keeps its original model prohibition even with the
    // same fully functional host factory and client.
    let (_, old_context, _, old_claim) = crate::native_admission_fixture::claim_with_source(
        rig.pool.clone(),
        "deterministic root",
        brassclaw_turns::run_profile::TRUSTED_INTERNAL_SOURCE_BINDING,
    )
    .await;
    let old_host = Arc::from(host_factory.create_host(&old_claim).await.unwrap());
    let old = MontyTaskHost::new(
        MontyTaskHandoff::new_claimed(
            AgentLoopDriverRunRequest {
                turn_id: old_context.turn_id,
                run_id: old_context.run_id,
                resolved_run_profile: old_claim.resolved_run_profile.clone(),
            },
            Arc::new(old_claim),
            old_host,
        )
        .unwrap(),
    );
    let denied = old
        .stream_model(LoopModelRequest {
            work_id: None,
            dispatch_policy: Default::default(),
            messages: Vec::new(),
            surface_version: None,
            model_preference: None,
            capability_view: None,
            resolved_messages: None,
        })
        .await
        .unwrap_err();
    assert_eq!(
        denied.safe_summary,
        "trusted internal turns require deterministic execution"
    );
    assert_eq!(calls.lock().await.len(), 2 * program.bindings().len());
    stop.cancel();
    server.await.unwrap();
    shutdown_shared_runtime(runtime).await.unwrap();
}
