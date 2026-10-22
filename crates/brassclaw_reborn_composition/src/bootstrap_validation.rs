//! Shared contained bootstrap admission and transport mechanics. No component
//! selection, expected behavior, approval or activation decisions live here.
use crate::bootstrap_reply_validation::invalid;
use crate::{RebornBuildError, pg_monty_admission::PgMontyAdmission};
use brassclaw_host_api::{AgentId, ProjectId, TenantId, ThreadId, UserId};
use brassclaw_loop_support::{
    RunCancellationFactory, RunStateLoopCancellationPort, ThreadBackedLoopTranscriptPort,
    TurnStateRunCancellationFactory,
};
use brassclaw_monty_host::process::{
    ProcessBoundary, ProcessLimits, ProcessSnapshot, RootBoot, TaskHandle, WorkerCommand,
};
use brassclaw_monty_host::transport_actor::{ActorLimits, TransportClient, TransportOwner};
use brassclaw_pg::PgPool;
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use brassclaw_threads::{
    AcceptInboundMessageRequest, EnsureThreadRequest, MessageContent, PgSessionThreadService,
    SessionThreadService, ThreadScope,
};
use brassclaw_turns::{
    AcceptedMessageRef, DefaultTurnCoordinator, IdempotencyKey, PgTurnStateStore,
    ReplyTargetBindingRef, SourceBindingRef, SubmitTurnRequest, TurnActor, TurnCoordinator,
    TurnLeaseToken, TurnRunnerId, TurnScope,
    run_profile::{AgentLoopDriverRunRequest, LoopRunContext, MontyTaskAttempt, MontyTaskHandoff},
    runner::{ClaimRunRequest, CompleteRunRequest, TurnRunTransitionPort},
};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};
use uuid::Uuid;
#[path = "bootstrap_reply_host.rs"]
mod host;

pub(super) const NAMESPACE: &str = "bootstrap-validation";

pub(super) async fn start(
    worker: &Path,
    boot: RootBoot,
    process: ProcessLimits,
    actor: ActorLimits,
    input: Value,
) -> Result<(TransportOwner, TransportClient, ProcessSnapshot, TaskHandle), RebornBuildError> {
    let (owner, ready) = TransportOwner::start(worker, boot, process, actor)
        .await
        .map_err(invalid)?;
    let transport = owner.client();
    let key = ready
        .work_waits
        .first()
        .ok_or_else(|| invalid("bootstrap work wait missing"))?
        .1;
    let mut root = transport
        .try_submit(WorkerCommand::Admit { key, task: input })
        .map_err(invalid)?
        .wait()
        .await
        .map_err(invalid)?
        .outcome
        .map_err(invalid)?;
    let task = root
        .admitted_task
        .ok_or_else(|| invalid("bootstrap task not admitted"))?;
    while let Some(ProcessBoundary::ControlYield { key }) = root.boundary {
        root = transport
            .try_submit(WorkerCommand::ResumeControl { key })
            .map_err(invalid)?
            .wait()
            .await
            .map_err(invalid)?
            .outcome
            .map_err(invalid)?;
    }
    if !matches!(root.boundary, Some(ProcessBoundary::HostCall { ref name, .. }) if name == "validation_hold")
    {
        return Err(invalid("bootstrap root did not reach contained hold"));
    }
    Ok((owner, transport, root, task))
}

pub(super) async fn close(
    owner: &mut TransportOwner,
    admitted: &Admitted,
) -> Result<(), RebornBuildError> {
    owner.request_termination();
    let exit = owner.join().await.map_err(invalid)?;
    if exit.exit_status.is_none() || exit.containment_error.is_some() || exit.reap_error.is_some() {
        return Err(invalid("bootstrap worker containment is unresolved"));
    }
    settle_closed(admitted).await
}

/// Close a contained private validation claim after its worker was reaped.
pub(super) async fn settle_closed(admitted: &Admitted) -> Result<(), RebornBuildError> {
    admitted
        .admission
        .settle(json!({"status":"failed","reason_kind":"bootstrap_validation_closed"}))
        .await
        .map_err(invalid)?;
    admitted
        .state
        .complete_run(CompleteRunRequest {
            run_id: admitted.attempt.run_id,
            runner_id: admitted.attempt.runner_id,
            lease_token: admitted.attempt.lease_token,
        })
        .await
        .map_err(invalid)?;
    Ok(())
}

pub(super) struct Admitted {
    pub(super) host: Arc<MontyTaskHost>,
    pub(super) input: Value,
    pub(super) admission: Arc<PgMontyAdmission>,
    pub(super) state: Arc<PgTurnStateStore>,
    pub(super) attempt: MontyTaskAttempt,
}
pub(super) async fn admit(
    pool: Arc<PgPool>,
    identity: &str,
    answer: &str,
) -> Result<Admitted, RebornBuildError> {
    prepare_admission(pool, identity, answer, AdmissionMode::Contained).await
}

/// Reserve a real installation-review parent without starting VM execution.
/// The owner must retain its captured catalogue, live claim and scheduled ports
/// before dispatch. This supplies no approval, activation or child authority by
/// itself; the native child owner still verifies every original reference.
#[cfg(test)]
pub(super) async fn reserve_child_review_parent(
    pool: Arc<PgPool>,
    identity: &str,
    reference: &str,
) -> Result<Admitted, RebornBuildError> {
    prepare_admission(pool, identity, reference, AdmissionMode::RetainedParent).await
}

#[derive(Clone, Copy)]
enum AdmissionMode {
    Contained,
    #[cfg(test)]
    RetainedParent,
}

async fn prepare_admission(
    pool: Arc<PgPool>,
    identity: &str,
    answer: &str,
    mode: AdmissionMode,
) -> Result<Admitted, RebornBuildError> {
    let tenant = TenantId::new(NAMESPACE).map_err(invalid)?;
    let agent = AgentId::new(identity).map_err(invalid)?;
    let actor = UserId::new("installation-owner").map_err(invalid)?;
    let (project, owner) = match mode {
        AdmissionMode::Contained => (Some(ProjectId::new("contained").map_err(invalid)?), None),
        #[cfg(test)]
        AdmissionMode::RetainedParent => (None, Some(actor.clone())),
    };
    let source_binding = match mode {
        AdmissionMode::Contained => "bootstrap-internal",
        #[cfg(test)]
        AdmissionMode::RetainedParent => {
            brassclaw_turns::run_profile::TRUSTED_INTERNAL_SOURCE_BINDING
        }
    };
    let conversation = ThreadId::new(format!("{identity}-{}", Uuid::new_v4())).map_err(invalid)?;
    let scope = ThreadScope {
        tenant_id: tenant.clone(),
        agent_id: agent.clone(),
        project_id: project.clone(),
        owner_user_id: owner,
    };
    let threads = Arc::new(PgSessionThreadService::new(pool.clone(), tenant.as_str()));
    threads
        .ensure_thread(EnsureThreadRequest {
            scope: scope.clone(),
            thread_id: Some(conversation.clone()),
            created_by_actor_id: actor.to_string(),
            title: Some("Internal bootstrap validation".into()),
            metadata_json: Some(json!({"bootstrap_validation":true}).to_string()),
        })
        .await
        .map_err(invalid)?;
    let accepted = threads
        .accept_inbound_message(AcceptInboundMessageRequest {
            scope: scope.clone(),
            thread_id: conversation.clone(),
            actor_id: actor.to_string(),
            source_binding_id: Some(source_binding.into()),
            reply_target_binding_id: Some(source_binding.into()),
            external_event_id: None,
            content: MessageContent::text(answer),
        })
        .await
        .map_err(invalid)?;
    let turn_scope = match mode {
        AdmissionMode::Contained => {
            TurnScope::new(tenant.clone(), Some(agent), project, conversation.clone())
        }
        #[cfg(test)]
        AdmissionMode::RetainedParent => TurnScope::new_with_owner(
            tenant.clone(),
            Some(agent),
            project,
            conversation.clone(),
            scope.owner_user_id.clone(),
        ),
    };
    let state = Arc::new(PgTurnStateStore::new(pool.clone(), tenant.as_str()));
    let coordinator = DefaultTurnCoordinator::new(state.clone());
    #[cfg(test)]
    let coordinator = match mode {
        AdmissionMode::Contained => coordinator,
        AdmissionMode::RetainedParent => coordinator.with_run_profile_resolver(Arc::new(
            brassclaw_reborn::planned_driver_factory::default_planned_run_profile_resolver()
                .map_err(invalid)?,
        )),
    };
    coordinator
        .submit_turn(SubmitTurnRequest {
            scope: turn_scope.clone(),
            actor: TurnActor::new(actor),
            accepted_message_ref: AcceptedMessageRef::new(format!("msg:{}", accepted.message_id))
                .map_err(invalid)?,
            source_binding_ref: SourceBindingRef::new(source_binding).map_err(invalid)?,
            reply_target_binding_ref: ReplyTargetBindingRef::new(source_binding)
                .map_err(invalid)?,
            requested_run_profile: None,
            idempotency_key: IdempotencyKey::new(Uuid::new_v4().to_string()).map_err(invalid)?,
            received_at: chrono::Utc::now(),
            requested_run_id: None,
            parent_run_id: None,
            subagent_depth: 0,
            spawn_tree_root_run_id: None,
        })
        .await
        .map_err(invalid)?;
    let claimed = state
        .claim_next_run(ClaimRunRequest {
            runner_id: TurnRunnerId::new(),
            lease_token: TurnLeaseToken::new(),
            scope_filter: Some(turn_scope.clone()),
        })
        .await
        .map_err(invalid)?
        .ok_or_else(|| invalid("bootstrap claim missing"))?;
    threads
        .mark_message_submitted(
            &scope,
            &conversation,
            accepted.message_id,
            claimed.state.turn_id.to_string(),
            claimed.state.run_id.to_string(),
        )
        .await
        .map_err(invalid)?;
    let context = LoopRunContext::new(
        turn_scope,
        claimed.state.turn_id,
        claimed.state.run_id,
        claimed.resolved_run_profile.clone(),
    )
    .with_actor(
        claimed
            .state
            .actor
            .clone()
            .ok_or_else(|| invalid("bootstrap actor missing"))?,
    )
    .with_accepted_message_ref(claimed.state.accepted_message_ref.clone());
    #[cfg(test)]
    let context = match mode {
        AdmissionMode::Contained => context,
        AdmissionMode::RetainedParent => context.with_trusted_internal_turn(),
    };
    let attempt = MontyTaskAttempt {
        run_id: claimed.state.run_id,
        runner_id: claimed.runner_id,
        lease_token: claimed.lease_token,
    };
    let admission = Arc::new(PgMontyAdmission::prepare(pool, &context, attempt).map_err(invalid)?);
    admission.persist_reservation().await.map_err(invalid)?;
    if matches!(mode, AdmissionMode::Contained) {
        admission.check_and_start().await.map_err(invalid)?;
    }
    let cancellation_owner = Arc::new(TurnStateRunCancellationFactory::new(state.clone()));
    let cancellation = RunStateLoopCancellationPort::new(
        cancellation_owner
            .handle_for_run(&context.scope, context.run_id)
            .await
            .map_err(invalid)?,
    );
    let host = Arc::new(host::BootstrapReplyHost {
        transcript: ThreadBackedLoopTranscriptPort::new(threads, scope, context.clone()),
        cancellation,
        _cancellation_owner: cancellation_owner,
    });
    let request = AgentLoopDriverRunRequest {
        turn_id: context.turn_id,
        run_id: context.run_id,
        resolved_run_profile: context.resolved_run_profile.clone(),
    };
    let handoff = match mode {
        AdmissionMode::Contained => MontyTaskHandoff::new(request, attempt, host),
        #[cfg(test)]
        AdmissionMode::RetainedParent => {
            MontyTaskHandoff::new_claimed(request, Arc::new(claimed), host)
        }
    }
    .map_err(invalid)?;
    let host = Arc::new(MontyTaskHost::new(handoff));
    Ok(Admitted {
        host,
        admission,
        state,
        attempt,
        input: json!({"conversation_id":conversation,"message_id":accepted.message_id,"turn_id":context.turn_id,"run_id":context.run_id,"user_input":answer,"history":[]}),
    })
}
