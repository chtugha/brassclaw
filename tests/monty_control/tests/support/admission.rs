//! Actual accepted transcript, durable run/claim and no-replay reservation.
//! Fixture preparation supplies no component activation or Tool permission.
use std::sync::Arc;

use brassclaw_host_api::{AgentId, ProjectId, TenantId, ThreadId, UserId};
use brassclaw_pg::PgPool;
use brassclaw_threads::{
    AcceptInboundMessageRequest, EnsureThreadRequest, MessageContent, PgSessionThreadService,
    SessionThreadService, ThreadScope,
};
use brassclaw_turns::{
    AcceptedMessageRef, DefaultTurnCoordinator, IdempotencyKey, PgTurnStateStore,
    ReplyTargetBindingRef, SourceBindingRef, SubmitTurnRequest, TurnActor, TurnCoordinator,
    TurnLeaseToken, TurnRunnerId, TurnScope,
    run_profile::{LoopRunContext, MontyTaskAttempt},
    runner::{ClaimRunRequest, TurnRunTransitionPort},
};
use serde_json::{Value, json};
use uuid::Uuid;

use super::pg_monty_admission::PgMontyAdmission;

pub(super) struct Admitted {
    pub input: Value,
    pub admission: Arc<PgMontyAdmission>,
    pub context: LoopRunContext,
    pub state: Arc<PgTurnStateStore>,
}
pub(super) async fn reserve(pool: Arc<PgPool>, user_input: &str) -> Admitted {
    reserve_with_claim(pool, user_input).await.0
}
pub(super) async fn reserve_with_claim(
    pool: Arc<PgPool>,
    user_input: &str,
) -> (Admitted, brassclaw_turns::runner::ClaimedTurnRun) {
    let (admitted, claimed) = reserve_unstarted_with_claim(pool, user_input).await;
    admitted.admission.check_and_start().await.unwrap();
    (admitted, claimed)
}

/// Match the production order: retain claim/address, bind task references,
/// then start the original admission before its first host operation.
pub(super) async fn reserve_unstarted_with_claim(
    pool: Arc<PgPool>,
    user_input: &str,
) -> (Admitted, brassclaw_turns::runner::ClaimedTurnRun) {
    let (input, context, state, claimed) =
        claim_with_source(pool.clone(), user_input, "draft-source").await;
    let attempt = MontyTaskAttempt {
        run_id: claimed.state.run_id,
        runner_id: claimed.runner_id,
        lease_token: claimed.lease_token,
    };
    let admission = Arc::new(
        PgMontyAdmission::reserve(pool, &context, attempt)
            .await
            .unwrap(),
    );
    (
        Admitted {
            input,
            admission,
            context,
            state,
        },
        claimed,
    )
}

/// Real accepted message and claim only. The actual global task factory, not
/// this fixture, reserves the admission in callers testing that boundary.
pub(super) async fn claim_with_source(
    pool: Arc<PgPool>,
    user_input: &str,
    source_binding: &str,
) -> (
    Value,
    LoopRunContext,
    Arc<PgTurnStateStore>,
    brassclaw_turns::runner::ClaimedTurnRun,
) {
    let tenant = TenantId::new("retained-invocation").unwrap();
    let agent = AgentId::new("draft-agent").unwrap();
    let project = ProjectId::new("draft-project").unwrap();
    let actor = UserId::new("draft-operator").unwrap();
    let conversation = ThreadId::new(format!("opaque-draft-{}", Uuid::new_v4())).unwrap();
    let scope = ThreadScope {
        tenant_id: tenant.clone(),
        agent_id: agent.clone(),
        project_id: Some(project.clone()),
        owner_user_id: None,
    };
    let threads = PgSessionThreadService::new(pool.clone(), tenant.as_str());
    threads
        .ensure_thread(EnsureThreadRequest {
            scope: scope.clone(),
            thread_id: Some(conversation.clone()),
            created_by_actor_id: actor.to_string(),
            title: None,
            metadata_json: None,
        })
        .await
        .unwrap();
    let accepted = threads
        .accept_inbound_message(AcceptInboundMessageRequest {
            scope: scope.clone(),
            thread_id: conversation.clone(),
            actor_id: actor.to_string(),
            source_binding_id: Some(source_binding.into()),
            reply_target_binding_id: Some("draft-reply".into()),
            external_event_id: None,
            content: MessageContent::text(user_input),
        })
        .await
        .unwrap();
    let turn_scope = TurnScope::new(
        tenant.clone(),
        Some(agent),
        Some(project),
        conversation.clone(),
    );
    let state = Arc::new(PgTurnStateStore::new(pool.clone(), tenant.as_str()));
    let coordinator = DefaultTurnCoordinator::new(state.clone());
    // Match the installation's actual internal host profile. The old generic
    // fixture below intentionally retains its historical default profile; it
    // is not a valid original claim for the strict native claimed handoff.
    let coordinator = if matches!(
        source_binding,
        brassclaw_turns::run_profile::INTERNAL_MODEL_SOURCE_BINDING
            | brassclaw_turns::run_profile::TRUSTED_INTERNAL_SOURCE_BINDING
    ) {
        coordinator.with_run_profile_resolver(Arc::new(
            brassclaw_reborn::planned_driver_factory::default_planned_run_profile_resolver()
                .unwrap(),
        ))
    } else {
        coordinator
    };
    coordinator
        .submit_turn(SubmitTurnRequest {
            scope: turn_scope.clone(),
            actor: TurnActor::new(actor),
            accepted_message_ref: AcceptedMessageRef::new(format!("msg:{}", accepted.message_id))
                .unwrap(),
            source_binding_ref: SourceBindingRef::new(source_binding).unwrap(),
            reply_target_binding_ref: ReplyTargetBindingRef::new("draft-reply").unwrap(),
            requested_run_profile: None,
            idempotency_key: IdempotencyKey::new(Uuid::new_v4().to_string()).unwrap(),
            received_at: chrono::Utc::now(),
            requested_run_id: None,
            parent_run_id: None,
            subagent_depth: 0,
            spawn_tree_root_run_id: None,
        })
        .await
        .unwrap();
    let claimed = state
        .claim_next_run(ClaimRunRequest {
            runner_id: TurnRunnerId::new(),
            lease_token: TurnLeaseToken::new(),
            scope_filter: Some(turn_scope.clone()),
        })
        .await
        .unwrap()
        .unwrap();
    threads
        .mark_message_submitted(
            &scope,
            &conversation,
            accepted.message_id,
            claimed.state.turn_id.to_string(),
            claimed.state.run_id.to_string(),
        )
        .await
        .unwrap();
    let mut context = LoopRunContext::new(
        turn_scope,
        claimed.state.turn_id,
        claimed.state.run_id,
        claimed.resolved_run_profile.clone(),
    )
    .with_actor(claimed.state.actor.clone().unwrap())
    .with_accepted_message_ref(claimed.state.accepted_message_ref.clone());
    if source_binding == brassclaw_turns::run_profile::TRUSTED_INTERNAL_SOURCE_BINDING {
        context = context.with_trusted_internal_turn();
    }
    let input = json!({"conversation_id":conversation,"message_id":accepted.message_id,
        "turn_id":context.turn_id,"run_id":context.run_id,"user_input":user_input,"history":[]});
    (input, context, state, claimed)
}
