//! One captured draft Match and named reply/history, then native binding.
//! These closed behavioral observations never activate a production catalogue.
use super::{
    catalogue_capture,
    global_recipe_ports::{MontyIntentSelection, MontyTaskCatalogue, SelectedMontyRecipe},
    global_task_factory::CapturedMontyCatalogue,
    pg_monty_admission::PgMontyAdmission,
    reply_workflows, retained_kernel,
};
use async_trait::async_trait;
use brassclaw_engine::{
    executor::{
        retained_recipe::{RetainedProgram, RetainedToolInvocation, RetainedToolPort},
        retained_source::InspectedRetainedProgram,
    },
    memory::{
        intent_system::{
            IntentResolution, RetainedIntentEligibility, resolve_catalogue_intent_in_transaction,
        },
        retained_tools::{RetainedToolBinding, RetainedToolProgram},
    },
};
use brassclaw_host_api::{
    CapabilityId, CapabilitySet, ExecutionContext, ExtensionId, MountView, ResourceEstimate,
    RuntimeKind, TrustClass, UserId,
};
use brassclaw_host_runtime::{
    RetainedFirstPartyCapability, RuntimeCapabilityOutcome, RuntimeCapabilityRequest,
};
use brassclaw_monty_host::{process::PortAnswer, service::PortFailure};
use brassclaw_pg::PgPool;
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio_postgres::IsolationLevel;

pub(super) struct CapturedValidationCatalogue {
    query: Option<String>,
    selected: Option<SelectedMontyRecipe>,
    outcome: Option<IntentResolution>,
    reply: Arc<InspectedRetainedProgram>,
    history: Arc<InspectedRetainedProgram>,
    pool: Arc<PgPool>,
    block_reply: bool,
}

impl CapturedValidationCatalogue {
    pub(super) async fn prepare(
        pool: Arc<PgPool>,
        host: Arc<MontyTaskHost>,
        draft: Option<Arc<RetainedToolProgram>>,
        reply: Arc<RetainedToolProgram>,
        block_reply: bool,
        query: &str,
    ) -> Self {
        let history = reply_workflows::history_program(pool.clone()).await;
        let mut client = pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        let eligible = RetainedIntentEligibility::from_instructions(
            &draft
                .iter()
                .map(|p| p.inputs().instruction())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let matched = resolve_catalogue_intent_in_transaction(
            &tx,
            &catalogue_capture::scope(&host),
            query,
            &eligible,
        )
        .await
        .unwrap();
        let mut originals = vec![reply.clone(), history.clone()];
        originals.extend(draft);
        let mut programs = catalogue_capture::programs(&tx, &originals, Some(&matched)).await;
        tx.commit().await.unwrap();
        drop(client);
        let selected = if let IntentResolution::Match {
            component_id,
            input_text,
            ..
        } = &matched
        {
            let program = programs.remove(component_id).unwrap();
            let inputs = program
                .inputs()
                .bind_variant_example(input_text, query, &json!({}))
                .unwrap();
            let normal_match =
                super::normal_match_evidence::NormalMatchEvidence::from_committed_match(
                    uuid::Uuid::new_v4(),
                    query,
                    &matched,
                    program.inputs().instruction(),
                    &inputs,
                )
                .expect("actual committed draft match");
            Some(SelectedMontyRecipe {
                normal_match: Some(Box::new(normal_match)),
                inspected: reply_workflows::inspect(program).await,
                inputs: Some(inputs.into()),
                tools: None,
            })
        } else {
            None
        };
        Self {
            query: Some(query.into()),
            selected,
            outcome: Some(matched),
            reply: reply_workflows::inspect(
                programs
                    .remove(&reply.inputs().instruction().recipe().uuid)
                    .unwrap(),
            )
            .await,
            history: reply_workflows::inspect(
                programs
                    .remove(&history.inputs().instruction().recipe().uuid)
                    .unwrap(),
            )
            .await,
            pool,
            block_reply,
        }
    }

    /// Explicit named child validation has no matcher operation at all.
    pub(super) async fn named(pool: Arc<PgPool>, reply: Arc<RetainedToolProgram>) -> Self {
        let history = reply_workflows::history_program(pool.clone()).await;
        let mut client = pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        let mut programs =
            catalogue_capture::programs(&tx, &[reply.clone(), history.clone()], None).await;
        tx.commit().await.unwrap();
        drop(client);
        Self {
            query: None,
            selected: None,
            outcome: None,
            reply: reply_workflows::inspect(
                programs
                    .remove(&reply.inputs().instruction().recipe().uuid)
                    .unwrap(),
            )
            .await,
            history: reply_workflows::inspect(
                programs
                    .remove(&history.inputs().instruction().recipe().uuid)
                    .unwrap(),
            )
            .await,
            pool,
            block_reply: false,
        }
    }
}

#[async_trait]
impl CapturedMontyCatalogue for CapturedValidationCatalogue {
    fn selected_workflow(
        &self,
    ) -> Result<
        Option<Arc<InspectedRetainedProgram>>,
        brassclaw_turns::run_profile::AgentLoopDriverError,
    > {
        Ok(self.selected.as_ref().map(|s| s.inspected.clone()))
    }
    fn captured_programs(&self) -> Vec<Arc<InspectedRetainedProgram>> {
        let mut programs = vec![self.reply.clone(), self.history.clone()];
        programs.extend(self.selected.as_ref().map(|s| s.inspected.clone()));
        programs
    }
    async fn bind(
        &self,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
    ) -> Result<Arc<dyn MontyTaskCatalogue>, brassclaw_turns::run_profile::AgentLoopDriverError>
    {
        let selected = if let Some(selected) = &self.selected {
            let RetainedProgram::Tools(program) = selected.inspected.program() else {
                panic!("captured native program")
            };
            let (runtime, _) = retained_kernel::runtime(program, "0:2").await;
            let mut bound = reply_workflows::copy_selection(selected);
            bound.tools = Some(Arc::new(JsonPort {
                runtime,
                program: program.clone(),
                admission: admission.clone(),
            }));
            Some(bound)
        } else {
            None
        };
        let reply = reply_workflows::prepare_named_reply(
            host,
            admission,
            self.reply.clone(),
            self.history.clone(),
            self.pool.clone(),
            self.block_reply,
        )
        .await;
        Ok(Arc::new(ValidationCatalogue {
            query: self.query.clone(),
            outcome: self.outcome.as_ref().map(|outcome| match outcome {
                IntentResolution::NoMatch => MontyIntentSelection::NoMatch,
                IntentResolution::Disambiguation { .. } => MontyIntentSelection::Disambiguation,
                IntentResolution::Match { .. } => MontyIntentSelection::Match(selected.unwrap()),
            }),
            reply,
        }))
    }
}

struct ValidationCatalogue {
    query: Option<String>,
    outcome: Option<MontyIntentSelection>,
    reply: reply_workflows::PreparedNamedReply,
}
fn failure() -> PortFailure {
    PortFailure::new("intent_resolution_failed").unwrap()
}
#[async_trait]
impl MontyTaskCatalogue for ValidationCatalogue {
    async fn refresh_command_qualification(&self) {}
    async fn resolve_intent(&self, query: &str) -> Result<MontyIntentSelection, PortFailure> {
        if self.query.as_deref() != Some(query) {
            return Err(failure());
        }
        match self.outcome.as_ref().ok_or_else(failure)? {
            MontyIntentSelection::Match(selected) => Ok(MontyIntentSelection::Match(
                reply_workflows::copy_selection(selected),
            )),
            MontyIntentSelection::NoMatch => Ok(MontyIntentSelection::NoMatch),
            MontyIntentSelection::Disambiguation => Ok(MontyIntentSelection::Disambiguation),
        }
    }
    async fn resolve_named_recipe(&self, name: &str) -> Result<SelectedMontyRecipe, PortFailure> {
        match name {
            "host-post-reply" => Ok(self.reply.select()),
            "host-save-history" => Ok(self.reply.history()),
            _ => Err(PortFailure::new("history_persistence_failed").unwrap()),
        }
    }
}

struct JsonPort {
    runtime: Arc<RetainedFirstPartyCapability>,
    program: Arc<RetainedToolProgram>,
    admission: Arc<PgMontyAdmission>,
}
#[async_trait]
impl RetainedToolPort for JsonPort {
    async fn dispatch(
        &self,
        invocation: RetainedToolInvocation<'_>,
        binding: &RetainedToolBinding,
        arguments: Value,
    ) -> PortAnswer {
        let retained = &self.program.bindings()[invocation.step_id()];
        assert_eq!(retained.tool(), binding.tool());
        assert_eq!(retained.python(), binding.python());
        let context = ExecutionContext::local_default(
            UserId::new("draft-operator").unwrap(),
            ExtensionId::new("draft-caller").unwrap(),
            RuntimeKind::FirstParty,
            TrustClass::FirstParty,
            CapabilitySet::default(),
            MountView::default(),
        )
        .unwrap();
        let record = self
            .admission
            .begin_tool_invocation(
                context.invocation_id,
                &self.program,
                invocation.step_id(),
                &arguments,
                invocation.data_limits(),
            )
            .await
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
        let answer = match actual {
            RuntimeCapabilityOutcome::Completed(actual) => PortAnswer::Return {
                value: actual.output.into(),
            },
            outcome => panic!("unexpected real kernel outcome: {outcome:?}"),
        };
        record.record_answer(&answer).await.unwrap();
        answer
    }
}
