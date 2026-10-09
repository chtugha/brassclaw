//! Whole draft Match -> IBS -> kernel reply -> named history Recipe, using the
//! actual global root and PostgreSQL. Not ordinary activation/startup evidence.
use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use brassclaw_engine::{
    executor::{
        retained_recipe::{RetainedProgram, RetainedToolInvocation, RetainedToolPort},
        retained_source::InspectedRetainedProgram,
    },
    memory::{
        intent_system::{
            InputClass, IntentResolution, IntentScope, IntentSource, RetainedIntentEligibility,
            resolve_catalogue_intent_in_transaction, seed_intent_input,
        },
        retained_instruction::{
            WorkflowClass, compile_matched_retained_recipe, compile_retained_recipe,
        },
        retained_tools::{RetainedToolBinding, RetainedToolProgram, prepare_retained_tool_program},
    },
};
use brassclaw_filesystem::{PostgresRootFilesystem, RootFilesystem};
use brassclaw_host_api::{
    CapabilityId, CapabilitySet, ExecutionContext, ExtensionId, ResourceEstimate, RuntimeKind,
    TrustClass,
};
use brassclaw_host_runtime::{RuntimeCapabilityOutcome, RuntimeCapabilityRequest};
use brassclaw_monty_host::{
    process::PortAnswer,
    service::{PortFailure, TaskInput, TaskPorts},
};
use brassclaw_skills::revision_store::PgComponentRevisionStore;
use brassclaw_threads::SessionThreadService;
use serde_json::{Value, json};
use tokio_postgres::IsolationLevel;

use crate::{
    ActorLimits, AdmissionInput, Duration, LiveMontyTaskSettings, MontyTaskHost,
    MontyTurnDriverPort, NativeTaskPortsFactory, PgPool, PgSessionThreadService, RecordingProvider,
    SOURCE, SelectedPrefix, TaskOutcome, admitted_with_text, global_monty_driver,
    global_monty_owner,
    global_recipe_ports::{MontyIntentSelection, MontyTaskCatalogue, SelectedMontyRecipe},
    global_task_factory, native_pg,
    pg_monty_admission::PgMontyAdmission,
    support,
};

#[path = "reply_programs.rs"]
mod programs;
#[path = "reply_kernel.rs"]
mod reply_kernel;

pub(crate) async fn reply_program(pool: Arc<PgPool>) -> Arc<RetainedToolProgram> {
    programs::program(&PgComponentRevisionStore::new(pool), false).await
}

pub(crate) struct PreparedNamedReply {
    selected: SelectedMontyRecipe,
    history: SelectedMontyRecipe,
    kernel: Arc<reply_kernel::Kernel>,
    block_on_selection: std::sync::atomic::AtomicBool,
}
impl PreparedNamedReply {
    pub(crate) fn history(&self) -> SelectedMontyRecipe {
        copy_selection(&self.history)
    }
    pub(crate) fn select(&self) -> SelectedMontyRecipe {
        if self
            .block_on_selection
            .swap(false, std::sync::atomic::Ordering::AcqRel)
        {
            // Actual settings publication after the model has answered; the
            // retained implementation must still consult this current rule.
            self.kernel.block_reply();
        }
        copy_selection(&self.selected)
    }
}
pub(crate) async fn prepare_named_reply(
    host: Arc<MontyTaskHost>,
    admission: Arc<PgMontyAdmission>,
    program: Arc<RetainedToolProgram>,
    pool: Arc<PgPool>,
    block_reply: bool,
) -> PreparedNamedReply {
    let filesystem = Arc::new(PostgresRootFilesystem::new((*pool).clone()));
    filesystem.run_migrations().await.unwrap();
    let history_program = programs::program(&PgComponentRevisionStore::new(pool), true).await;
    let kernel = Arc::new(reply_kernel::kernel(
        host.clone(),
        program.bindings()["0:2"].tool().uuid,
        history_program.bindings()["0:3"].tool().uuid,
        filesystem,
    ));
    let tools = Arc::new(Tools {
        host: host.clone(),
        program: program.clone(),
        admission: admission.clone(),
        kernel: kernel.clone(),
        block_history: block_reply,
    });
    let selected = SelectedMontyRecipe {
        normal_match: None,
        inspected: Arc::new(
            InspectedRetainedProgram::inspect(RetainedProgram::Tools(program), support::worker())
                .await
                .unwrap(),
        ),
        inputs: None,
        tools: Some(tools),
    };
    let history_tools = Arc::new(Tools {
        host,
        program: history_program.clone(),
        admission,
        kernel: kernel.clone(),
        block_history: false,
    });
    let history = SelectedMontyRecipe {
        normal_match: None,
        inspected: Arc::new(
            InspectedRetainedProgram::inspect(
                RetainedProgram::Tools(history_program), support::worker(),
            ).await.unwrap(),
        ),
        inputs: None,
        tools: Some(history_tools),
    };
    PreparedNamedReply {
        selected,
        history,
        kernel,
        block_on_selection: std::sync::atomic::AtomicBool::new(block_reply),
    }
}

struct DraftProvider {
    pool: Arc<PgPool>,
    reply: Arc<RetainedToolProgram>,
    history: Arc<RetainedToolProgram>,
    filesystem: Arc<PostgresRootFilesystem>,
    block_history: bool,
    observed_kernel: Arc<std::sync::Mutex<Option<Arc<reply_kernel::Kernel>>>>,
}
struct Catalogue {
    query: String,
    selected: MontyIntentSelection,
    reply: SelectedMontyRecipe,
    history: SelectedMontyRecipe,
    kernel: Arc<reply_kernel::Kernel>,
    block_history: bool,
}
fn failure() -> PortFailure {
    PortFailure::new("recipe_composition_failed").unwrap()
}

#[async_trait]
impl global_task_factory::MontyCatalogueProvider for DraftProvider {
    async fn capture(
        &self,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
        input: &TaskInput,
    ) -> Result<Arc<dyn MontyTaskCatalogue>, brassclaw_turns::run_profile::AgentLoopDriverError>
    {
        // This validator's explicit immutable graph membership is not a claim
        // of production approval. Matching and both assemblies nevertheless
        // read their exact records in one real consistent database snapshot.
        let context = host.run_context();
        let scope = IntentScope {
            tenant_id: context.scope.tenant_id.to_string(),
            user_id: context.actor.as_ref().unwrap().user_id.to_string(),
            agent_id: context.scope.agent_id.as_ref().unwrap().to_string(),
            project_id: context.scope.project_id.as_ref().unwrap().to_string(),
        };
        let mut client = self.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        let eligible =
            RetainedIntentEligibility::from_instructions(&[self.reply.inputs().instruction()])
                .unwrap();
        let matched =
            resolve_catalogue_intent_in_transaction(&tx, &scope, &input.user_input, &eligible)
                .await
                .unwrap();
        let mut retained = HashMap::new();
        for draft in [&self.reply, &self.history] {
            let instruction = draft.inputs().instruction();
            let refs: Vec<_> = instruction
                .snapshot()
                .revisions()
                .values()
                .map(|r| r.reference())
                .collect();
            let snapshot = Arc::new(
                PgComponentRevisionStore::read_exact_in_transaction(
                    &tx,
                    &[instruction.recipe().uuid],
                    &refs,
                )
                .await
                .unwrap(),
            );
            let instruction = if draft.inputs().instruction().recipe()
                == self.reply.inputs().instruction().recipe()
                && matches!(&matched, IntentResolution::Match { .. })
            {
                compile_matched_retained_recipe(snapshot, &matched, WorkflowClass::Deterministic)
                    .unwrap()
            } else {
                compile_retained_recipe(
                    snapshot,
                    instruction.recipe().uuid,
                    "selected",
                    WorkflowClass::Deterministic,
                )
                .unwrap()
            };
            retained.insert(
                instruction.recipe().uuid,
                Arc::new(prepare_retained_tool_program(instruction).unwrap()),
            );
        }
        tx.commit().await.unwrap();
        let kernel = Arc::new(reply_kernel::kernel(
            host.clone(),
            self.reply.bindings()["0:2"].tool().uuid,
            self.history.bindings()["0:3"].tool().uuid,
            self.filesystem.clone(),
        ));
        *self.observed_kernel.lock().unwrap() = Some(kernel.clone());
        for (wrong_thread, payload) in [
            (true, json!({"answer":"must not publish"})),
            (false, json!({"answer":"   "})),
            (false, json!({"answer":"must not publish","extra":true})),
        ] {
            let mut addressed = execution(&host, &kernel);
            if wrong_thread {
                addressed.thread_id =
                    Some(brassclaw_host_api::ThreadId::new("another-conversation").unwrap());
                addressed.resource_scope.thread_id = addressed.thread_id.clone();
                addressed.validate().unwrap();
            }
            let denied = kernel.handles["host.post_reply"]
                .invoke(RuntimeCapabilityRequest::new(
                    addressed,
                    CapabilityId::new("host.post_reply").unwrap(),
                    ResourceEstimate::default(),
                    payload,
                    reply_kernel::trust(),
                ))
                .await
                .unwrap();
            assert!(
                matches!(denied, RuntimeCapabilityOutcome::Failed(_)),
                "invalid reply address/payload reached publication: {denied:?}"
            );
            assert!(host.finalized_reply_ref().is_none());
        }
        let mut inspected = HashMap::new();
        for (id, program) in retained {
            let tools = Arc::new(Tools {
                host: host.clone(),
                program: program.clone(),
                admission: admission.clone(),
                kernel: kernel.clone(),
                block_history: self.block_history,
            });
            let inspected_program = Arc::new(
                InspectedRetainedProgram::inspect(
                    RetainedProgram::Tools(program),
                    support::worker(),
                )
                .await
                .unwrap(),
            );
            inspected.insert(
                id,
                SelectedMontyRecipe {
                    normal_match: None,
                    inspected: inspected_program,
                    inputs: None,
                    tools: Some(tools),
                },
            );
        }
        let reply = copy_selection(&inspected[&self.reply.inputs().instruction().recipe().uuid]);
        let selected = match &matched {
            IntentResolution::NoMatch => MontyIntentSelection::NoMatch,
            IntentResolution::Disambiguation { .. } => MontyIntentSelection::Disambiguation,
            IntentResolution::Match {
                component_id,
                input_text,
                ..
            } => {
                let mut selection = inspected.remove(component_id).unwrap();
                selection.inputs = Some(
                    selection
                        .inspected
                        .program()
                        .inputs()
                        .bind_variant_example(&input_text, &input.user_input, &json!({}))
                        .unwrap(),
                );
                // This identifies this validator's committed draft snapshot,
                // not an activated/approved production catalogue generation.
                selection.normal_match = crate::normal_match_evidence::NormalMatchEvidence::from_committed_match(
                    uuid::Uuid::new_v4(), &input.user_input, &matched,
                    selection.inspected.program().inputs().instruction(),
                    selection.inputs.as_ref().unwrap(),
                ).map(Box::new);
                assert!(selection.normal_match.is_some());
                MontyIntentSelection::Match(selection)
            }
        };
        Ok(Arc::new(Catalogue {
            query: input.user_input.clone(),
            selected,
            reply,
            history: inspected
                .remove(&self.history.inputs().instruction().recipe().uuid)
                .unwrap(),
            kernel,
            block_history: self.block_history,
        }))
    }
}
#[async_trait]
impl MontyTaskCatalogue for Catalogue {
    // This explicit draft validator never advertises an activated MCP command.
    async fn refresh_command_qualification(&self) {}

    async fn resolve_intent(&self, query: &str) -> Result<MontyIntentSelection, PortFailure> {
        if query != self.query {
            return Err(failure());
        }
        Ok(match &self.selected {
            MontyIntentSelection::NoMatch => MontyIntentSelection::NoMatch,
            MontyIntentSelection::Disambiguation => MontyIntentSelection::Disambiguation,
            MontyIntentSelection::Match(recipe) => {
                MontyIntentSelection::Match(copy_selection(recipe))
            }
        })
    }
    async fn resolve_named_recipe(&self, name: &str) -> Result<SelectedMontyRecipe, PortFailure> {
        if name == "host-post-reply" {
            return Ok(copy_selection(&self.reply));
        }
        if name != "host-save-history" {
            return Err(failure());
        }
        if self.block_history {
            self.kernel.block_memory();
        }
        // Selection was retained with the first Recipe, before any effects;
        // never look up a replacement version or a fresh Tool implementation.
        Ok(copy_selection(&self.history))
    }
}
fn copy_selection(recipe: &SelectedMontyRecipe) -> SelectedMontyRecipe {
    SelectedMontyRecipe {
        normal_match: recipe.normal_match.clone(),
        inspected: recipe.inspected.clone(),
        inputs: recipe.inputs.clone(),
        tools: recipe.tools.clone(),
    }
}
struct Tools {
    host: Arc<MontyTaskHost>,
    program: Arc<RetainedToolProgram>,
    admission: Arc<PgMontyAdmission>,
    kernel: Arc<reply_kernel::Kernel>,
    block_history: bool,
}
fn execution(host: &MontyTaskHost, kernel: &reply_kernel::Kernel) -> ExecutionContext {
    let bound = host.run_context();
    let mut context = ExecutionContext::local_default(
        bound.actor.as_ref().unwrap().user_id.clone(),
        ExtensionId::new("draft-workflow").unwrap(),
        RuntimeKind::FirstParty,
        TrustClass::FirstParty,
        CapabilitySet::default(),
        kernel.mounts.clone(),
    )
    .unwrap();
    context.tenant_id = bound.scope.tenant_id.clone();
    context.agent_id = bound.scope.agent_id.clone();
    context.project_id = bound.scope.project_id.clone();
    context.thread_id = Some(bound.thread_id.clone());
    context.resource_scope.tenant_id = context.tenant_id.clone();
    context.resource_scope.agent_id = context.agent_id.clone();
    context.resource_scope.project_id = context.project_id.clone();
    context.resource_scope.thread_id = context.thread_id.clone();
    context.validate().unwrap();
    context
}
#[async_trait]
impl RetainedToolPort for Tools {
    async fn dispatch(
        &self,
        invocation: RetainedToolInvocation<'_>,
        binding: &RetainedToolBinding,
        arguments: Value,
    ) -> PortAnswer {
        let selected = &self.program.bindings()[invocation.step_id()];
        assert_eq!(selected.tool(), binding.tool());
        assert_eq!(selected.python(), binding.python());
        let capability = CapabilityId::new(binding.capability_id()).unwrap();
        assert_eq!(
            self.kernel.policy.tool_identity(&capability).unwrap(),
            Some(binding.tool().uuid)
        );
        let record = self
            .admission
            .begin_tool_invocation(&self.program, invocation.step_id(), &arguments)
            .await
            .unwrap();
        let actual = self.kernel.handles[binding.capability_id()]
            .invoke(RuntimeCapabilityRequest::new(
                execution(&self.host, &self.kernel),
                capability,
                ResourceEstimate::default(),
                arguments,
                reply_kernel::trust(),
            ))
            .await
            .unwrap();
        let answer = match &actual {
            RuntimeCapabilityOutcome::Completed(actual) => PortAnswer::Return {
                value: actual.output.clone(),
            },
            RuntimeCapabilityOutcome::Failed(failed) => PortAnswer::TerminalError {
                reason_kind: format!("retained_tool_{}", failed.kind.as_str()),
            },
            other => panic!("unexpected actual kernel gate: {other:?}"),
        };
        record.record_answer(&answer).await.unwrap();
        if !self.block_history {
            assert!(
                matches!(actual, RuntimeCapabilityOutcome::Completed(_)),
                "actual retained kernel failure: {actual:?}"
            );
        }
        answer
    }
}

#[tokio::test]
async fn global_match_posts_reply_and_runs_history_without_model_replay() {
    whole_match(false, false).await;
}
#[tokio::test]
async fn live_history_policy_block_preserves_published_reply_without_replay() {
    whole_match(true, false).await;
}

#[tokio::test]
async fn global_root_cannot_complete_after_skipping_selected_history() {
    whole_match(false, true).await;
}

async fn whole_match(block_history: bool, skip_history: bool) {
    // Each filtered case must initialize its actual host prerequisite instead
    // of depending on another test having populated the process-wide prompt.
    brassclaw_reborn::loop_driver_host::init_compaction_summarizer(
        include_str!(
            "../../../../crates/brassclaw_loop_support/prompts/compaction_summarizer_fresh.md"
        )
        .to_owned(),
    );
    let database = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(database.pool.clone());
    let reply = programs::program(&store, false).await;
    let history = programs::program(&store, true).await;
    seed_intent_input(
        &database.pool,
        &IntentScope {
            tenant_id: "native-global-host".into(),
            user_id: "global-operator".into(),
            agent_id: "global-agent".into(),
            project_id: "global-project".into(),
        },
        "reply %",
        InputClass::Partial,
        reply.inputs().instruction().recipe().uuid,
        21,
        IntentSource::Seeded,
        Some("0:1-0:E"),
    )
    .await
    .unwrap();
    let filesystem = Arc::new(PostgresRootFilesystem::new((*database.pool).clone()));
    filesystem.run_migrations().await.unwrap();
    let provider = Arc::new(RecordingProvider::default());
    let prefix = Arc::new(SelectedPrefix("actual draft validation prefix".into()));
    // Explicit negative validation source: it claims completion after the real
    // reply/formatter but skips the selected memory feed. It is never approved
    // or supplied to ordinary startup. The real service must reject its claim.
    let marker = "            step_id = node[\"step_id\"]\n            # Opaque";
    assert_eq!(SOURCE.matches(marker).count(), 1);
    let source = if skip_history {
        SOURCE.replacen(marker, "            step_id = node[\"step_id\"]\n            if step_id == \"0:3\":\n                return {\"returned\": True, \"value\": None}\n            # Opaque", 1)
    } else {
        SOURCE.to_owned()
    };
    let boot = support::boot(&source);
    let live = LiveMontyTaskSettings::new(boot.task_settings.into()).unwrap();
    let mut owner = global_monty_owner::GlobalMontyOwner::start(
        &database.pool,
        support::worker(),
        global_monty_owner::GlobalServiceConfig {
            ownership: global_monty_owner::OwnershipLimits::from_execution(Default::default()),
            boot,
            process: support::limits(),
            live,
            actor: ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: 4 * support::limits().max_frame_bytes,
                max_control_unclaimed: 4,
                max_control_reserved_frame_bytes: 2 * support::limits().max_frame_bytes,
            },
            queue_capacity: 8,
            queue_bytes: 64 * 1024 * 1024,
            max_pending_settings: 8,
            max_retained_attempts: 1,
        },
    )
    .await
    .unwrap();
    let observed_kernel = Arc::new(std::sync::Mutex::new(None));
    let factory = Arc::new(NativeTaskPortsFactory::with_catalogue(
        database.pool.clone(),
        owner.ownership_check(),
        owner.client(),
        Arc::new(DraftProvider {
            pool: database.pool.clone(),
            reply,
            history,
            filesystem: filesystem.clone(),
            block_history,
            observed_kernel: observed_kernel.clone(),
        }),
    ));
    let threads = Arc::new(PgSessionThreadService::new(
        database.pool.clone(),
        "native-global-host",
    ));
    let driver =
        global_monty_driver::GlobalMontyDriver::new(owner.client(), threads, factory.clone())
            .unwrap();
    let answer = "Literal 'quotes' Ü {{vars.answer}} host.forbidden()";
    let query = format!("reply {answer}");
    let (_, handoff, threads, scope, _) = admitted_with_text(
        database.pool.clone(),
        provider.clone(),
        AdmissionInput {
            name: "actual-reply-workflow",
            text: &query,
        },
        prefix,
        0,
        owner.client().live_task_settings(),
        None,
    )
    .await;
    let (request, attempt, host) = handoff.into_parts();
    let handoff = crate::MontyTaskHandoff::new(request, attempt, host).unwrap();
    let result = tokio::time::timeout(Duration::from_secs(20), driver.drive_turn(handoff))
        .await
        .unwrap();
    let host = factory.last_host.lock().unwrap().clone().unwrap();
    let reference = host
        .finalized_reply_ref()
        .expect("actual transcript publication");
    let transcript = threads
        .list_thread_history(brassclaw_threads::ThreadHistoryRequest {
            scope: scope.clone(),
            thread_id: host.run_context().thread_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(transcript.messages.len(), 2);
    assert_eq!(transcript.messages[1].content.as_deref(), Some(answer));
    assert!(provider.requests.lock().unwrap().is_empty());
    if skip_history {
        assert!(matches!(
            result,
            Err(brassclaw_turns::run_profile::AgentLoopDriverError::Failed { reason_kind }) if reason_kind == "monty_service_reconciliation_required"
        ));
        assert!(
            factory.last_receipt.lock().unwrap().is_none(),
            "no successful task receipt may be manufactured"
        );
        assert!(driver.take_settlement(attempt).is_err());
        assert!(factory.inner.take_failed_settlement(attempt).is_err());
        let client = database.pool.get().await.unwrap();
        let admission = client
            .query_one(
                "SELECT phase,outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
                &[&attempt.run_id.as_uuid()],
            )
            .await
            .unwrap();
        assert_eq!(admission.get::<_, &str>(0), "started");
        assert!(admission.get::<_, Option<Value>>(1).is_none());
        assert_eq!(client.query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1 AND phase='answered'", &[&attempt.run_id.as_uuid()]).await.unwrap().get::<_, i64>(0), 1);
        assert_eq!(
            client
                .query_one("SELECT count(*) FROM root_filesystem_entries", &[])
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
        drop(client);
        owner.request_shutdown();
        let exit = tokio::time::timeout(Duration::from_secs(10), owner.join())
            .await
            .unwrap()
            .unwrap();
        let service = exit.service.unwrap();
        assert_eq!(
            service.failure,
            Some(brassclaw_monty_host::service::ServiceFailure::InvalidPortResult)
        );
        assert_eq!(
            service.tasks.len(),
            1,
            "retain actual task/port evidence for reconciliation"
        );
        assert!(service.tasks[0].reported_outcome.is_none());
        assert_eq!(host.finalized_reply_ref(), Some(reference));
        match exit.ownership {
            global_monty_owner::OwnershipSettlement::ReleaseAttempt(result) => result.unwrap(),
            global_monty_owner::OwnershipSettlement::Quarantined(_) => {
                panic!("actual worker containment and reaping required")
            }
        }
        return;
    }
    // Normal acknowledged completion releases the driver's reservation; the
    // observer retained the actual service receipt supplied to settlement.
    let receipt = factory.last_receipt.lock().unwrap().clone().unwrap();
    if block_history {
        let (actual_host, actual_receipt, control, _driver_retention) =
            driver.take_settlement(attempt).unwrap().unwrap();
        assert!(Arc::ptr_eq(&host, &actual_host));
        assert!(Arc::ptr_eq(&receipt, &actual_receipt));
        assert!(control.receipt().unwrap().is_ok());
    } else {
        assert!(driver.take_settlement(attempt).unwrap().is_none());
    }
    assert!(receipt.withheld.is_empty());
    assert!(receipt.accounting.is_some());
    let client = database.pool.get().await.unwrap();
    let audit = client
        .query_one(
            "SELECT phase,outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&attempt.run_id.as_uuid()],
        )
        .await
        .unwrap();
    assert_eq!(audit.get::<_, &str>(0), "settled");
    if !block_history {
        super::admission_integrity::reject_terminal_rewrite(&database.pool, attempt.run_id).await;
    }
    let report = super::assert_settlement_outcome(
        audit.get(1),
        if block_history {
            json!({"status":"failed","reason_kind":"recipe_execution_failed"})
        } else {
            json!({"status":"completed","reply_ref":reference.as_str()})
        },
    );
    assert_eq!(report["recipes"].as_array().unwrap().len(), 2);
    assert_eq!(report["recipes"][0]["completed_steps"], json!(["0:2"]));
    assert_eq!(report["recipes"][0]["complete"], true);
    assert_eq!(report["all_selected_recipes_complete"], !block_history);
    if block_history {
        assert_eq!(report["recipes"][1]["completed_steps"], json!(["0:1"]));
        assert_eq!(report["recipes"][1]["failed_step"], "0:3");
        assert_eq!(report["recipes"][1]["complete"], false);
    } else {
        assert_eq!(
            report["recipes"][1]["completed_steps"],
            json!(["0:1", "0:3"])
        );
        assert!(report["recipes"][1]["failed_step"].is_null());
        assert_eq!(report["recipes"][1]["complete"], true);
    }
    let rows = client.query("SELECT phase, attempt_count, answer_bytes FROM brassclaw_monty_tool_invocations WHERE run_id=$1 ORDER BY step_id",
        &[&attempt.run_id.as_uuid()]).await.unwrap();
    assert_eq!(rows.len(), 2);
    for row in &rows {
        assert_eq!(row.get::<_, &str>(0), "answered");
        assert_eq!(row.get::<_, i16>(1), 1);
    }
    let reply_answer: Value = serde_json::from_str(rows[0].get(2)).unwrap();
    assert_eq!(
        reply_answer,
        json!({"kind":"return","value":reference.as_str()})
    );
    let memory_answer: Value = serde_json::from_str(rows[1].get(2)).unwrap();
    if block_history {
        let error = result.unwrap_err();
        assert!(
            matches!(error,brassclaw_turns::run_profile::AgentLoopDriverError::Failed { reason_kind } if reason_kind=="recipe_execution_failed")
        );
        assert!(matches!(receipt.outcome, TaskOutcome::Failed { .. }));
        assert_eq!(memory_answer["kind"], "terminal_error");
        let (retained_host, _, ports, retained_receipt, _factory_retention) = factory
            .inner
            .take_failed_settlement(attempt)
            .unwrap()
            .unwrap();
        assert!(Arc::ptr_eq(&host, &retained_host));
        assert!(Arc::ptr_eq(&receipt, &retained_receipt));
        ports.fence();
        let stored: i64 = client
            .query_one("SELECT count(*) FROM root_filesystem_entries", &[])
            .await
            .unwrap()
            .get(0);
        assert_eq!(stored, 0);
    } else {
        assert!(matches!(
            result.unwrap(),
            brassclaw_turns::LoopExit::Completed(_)
        ));
        assert!(
            matches!(&receipt.outcome,TaskOutcome::Completed { reply_ref } if reply_ref==reference.as_str())
        );
        let path = memory_answer["value"]["path"].as_str().unwrap();
        let context = host.run_context();
        let stored = filesystem
            .read_file(
                &brassclaw_host_api::VirtualPath::new(format!(
                    "/memory/tenants/{}/users/{}/agents/{}/projects/{}/{path}",
                    context.scope.tenant_id,
                    context.actor.as_ref().unwrap().user_id,
                    context.scope.agent_id.as_ref().unwrap(),
                    context.scope.project_id.as_ref().unwrap(),
                ))
                .unwrap(),
            )
            .await
            .unwrap();
        let content = String::from_utf8(stored).unwrap();
        assert!(content.ends_with('\n'));
        assert_eq!(content.lines().count(), 1);
        assert_eq!(serde_json::from_str::<Value>(&content).unwrap(), json!({
            "format":"completed-turn/1", "user_input":query,
            "answer":answer, "reply_ref":reference.as_str(),
        }));
    }
    drop(client);
    // Retained implementation ownership cannot reopen a completed/failed
    // attempt. This uses the original real handler through the kernel again,
    // with a valid payload and still-enabled Tool rule, after explicit fencing.
    host.fence_dispatch();
    let kernel = observed_kernel.lock().unwrap().take().unwrap();
    let denied = kernel.handles["host.post_reply"]
        .invoke(RuntimeCapabilityRequest::new(
            execution(&host, &kernel),
            CapabilityId::new("host.post_reply").unwrap(),
            ResourceEstimate::default(),
            json!({"answer":"must never replay"}),
            reply_kernel::trust(),
        ))
        .await
        .unwrap();
    assert!(matches!(denied,RuntimeCapabilityOutcome::Failed(failed)
        if failed.kind==brassclaw_host_runtime::RuntimeFailureKind::OperationFailed));
    assert_eq!(host.finalized_reply_ref(), Some(reference));
    assert_eq!(
        threads
            .list_thread_history(brassclaw_threads::ThreadHistoryRequest {
                scope,
                thread_id: host.run_context().thread_id.clone(),
            })
            .await
            .unwrap()
            .messages
            .len(),
        2
    );
    owner.request_shutdown();
    let exit = tokio::time::timeout(Duration::from_secs(10), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert!(exit.service.unwrap().tasks.is_empty());
    match exit.ownership {
        global_monty_owner::OwnershipSettlement::ReleaseAttempt(release) => release.unwrap(),
        global_monty_owner::OwnershipSettlement::Quarantined(_) => {
            panic!("actual clean worker exit required")
        }
    }
}
