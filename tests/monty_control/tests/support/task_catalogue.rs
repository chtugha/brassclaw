//! Real matcher/revision/IBS/kernel caller for unapproved behavioral validation.
//! No producer approval, activated catalogue or ordinary startup is simulated.
use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_engine::{
    executor::{
        retained_recipe::{RetainedProgram, RetainedToolInvocation, RetainedToolPort},
        retained_source::InspectedRetainedProgram,
    },
    memory::{
        intent_system::{
            IntentResolution, IntentScope, RetainedIntentEligibility,
            resolve_catalogue_intent_in_transaction,
        },
        retained_instruction::{WorkflowClass, compile_matched_retained_recipe},
        retained_tools::{RetainedToolBinding, RetainedToolProgram, prepare_retained_tool_program},
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
use brassclaw_skills::revision_store::PgComponentRevisionStore;
use serde_json::{Value, json};
use tokio_postgres::IsolationLevel;

use crate::{
    global_recipe_ports::{MontyIntentSelection, MontyTaskCatalogue, SelectedMontyRecipe},
    pg_monty_admission::PgMontyAdmission,
    retained_kernel, support,
};

pub struct ValidationCatalogue {
    pool: Arc<PgPool>,
    host: Arc<MontyTaskHost>,
    draft: Option<Arc<RetainedToolProgram>>,
    admission: Arc<PgMontyAdmission>,
    reply: crate::reply_workflows::PreparedNamedReply,
}
impl ValidationCatalogue {
    pub async fn new(
        pool: Arc<PgPool>,
        host: Arc<MontyTaskHost>,
        draft: Option<Arc<RetainedToolProgram>>,
        admission: Arc<PgMontyAdmission>,
        reply: Arc<RetainedToolProgram>,
        block_reply: bool,
    ) -> Self {
        let reply = crate::reply_workflows::prepare_named_reply(
            host.clone(),
            admission.clone(),
            reply,
            pool.clone(),
            block_reply,
        )
        .await;
        Self {
            pool,
            host,
            draft,
            admission,
            reply,
        }
    }
}
fn failure() -> PortFailure {
    PortFailure::new("intent_resolution_failed").unwrap()
}
#[async_trait]
impl MontyTaskCatalogue for ValidationCatalogue {
    async fn resolve_intent(&self, query: &str) -> Result<MontyIntentSelection, PortFailure> {
        let context = self.host.run_context();
        let scope = IntentScope {
            tenant_id: context.scope.tenant_id.to_string(),
            user_id: context
                .actor
                .as_ref()
                .ok_or_else(failure)?
                .user_id
                .to_string(),
            agent_id: context
                .scope
                .agent_id
                .as_ref()
                .ok_or_else(failure)?
                .to_string(),
            project_id: context
                .scope
                .project_id
                .as_ref()
                .ok_or_else(failure)?
                .to_string(),
        };
        let mut client = self.pool.get().await.map_err(|_| failure())?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| failure())?;
        let instructions: Vec<_> = self
            .draft
            .iter()
            .map(|program| program.inputs().instruction())
            .collect();
        // Explicit draft membership is restricted to this behavioral validator.
        // Production membership must instead come from coherent activation.
        let eligible =
            RetainedIntentEligibility::from_instructions(&instructions).map_err(|_| failure())?;
        let matched = resolve_catalogue_intent_in_transaction(&tx, &scope, query, &eligible)
            .await
            .map_err(|_| failure())?;
        let result = match &matched {
            IntentResolution::NoMatch => MontyIntentSelection::NoMatch,
            IntentResolution::Disambiguation { .. } => MontyIntentSelection::Disambiguation,
            IntentResolution::Match {
                component_id,
                input_text,
                ..
            } => {
                let draft = self.draft.as_ref().ok_or_else(failure)?;
                if *component_id != draft.inputs().instruction().recipe().uuid {
                    return Err(failure());
                }
                let refs: Vec<_> = draft
                    .inputs()
                    .instruction()
                    .snapshot()
                    .revisions()
                    .values()
                    .map(|r| r.reference())
                    .collect();
                let snapshot = Arc::new(
                    PgComponentRevisionStore::read_exact_in_transaction(
                        &tx,
                        &[*component_id],
                        &refs,
                    )
                    .await
                    .map_err(|_| failure())?,
                );
                let instruction = compile_matched_retained_recipe(
                    snapshot,
                    &matched,
                    WorkflowClass::Deterministic,
                )
                .map_err(|_| failure())?;
                let program =
                    Arc::new(prepare_retained_tool_program(instruction).map_err(|_| failure())?);
                let inputs = program
                    .inputs()
                    .bind_variant_example(input_text, query, &json!({}))
                    .map_err(|_| failure())?;
                let (runtime, _) = retained_kernel::runtime(&program, "0:2").await;
                let tools = Arc::new(JsonPort {
                    runtime,
                    program: program.clone(),
                    admission: self.admission.clone(),
                });
                // Commit the coherent read before spawning the bounded utility.
                tx.commit().await.map_err(|_| failure())?;
                let inspected = Arc::new(
                    InspectedRetainedProgram::inspect(
                        RetainedProgram::Tools(program),
                        support::worker(),
                    )
                    .await
                    .map_err(|_| failure())?,
                );
                return Ok(MontyIntentSelection::Match(SelectedMontyRecipe {
                    inspected,
                    inputs: Some(inputs),
                    tools: Some(tools),
                }));
            }
        };
        tx.commit().await.map_err(|_| failure())?;
        Ok(result)
    }
    async fn resolve_named_recipe(&self, name: &str) -> Result<SelectedMontyRecipe, PortFailure> {
        if name == "host-post-reply" {
            return Ok(self.reply.select());
        }
        // This validation catalogue has no approved named history workflow.
        Err(PortFailure::new("history_persistence_failed").unwrap())
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
        let record = self
            .admission
            .begin_tool_invocation(&self.program, invocation.step_id(), &arguments)
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
        let answer = match actual {
            RuntimeCapabilityOutcome::Completed(actual) => PortAnswer::Return {
                value: actual.output,
            },
            outcome => panic!("unexpected real kernel outcome: {outcome:?}"),
        };
        record.record_answer(&answer).await.unwrap();
        answer
    }
}
