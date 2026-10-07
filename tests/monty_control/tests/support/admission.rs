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

use crate::pg_monty_admission::PgMontyAdmission;

pub struct Admitted {
    pub input: Value,
    pub admission: Arc<PgMontyAdmission>,
    pub context: LoopRunContext,
    pub state: Arc<PgTurnStateStore>,
}
pub async fn reserve(pool: Arc<PgPool>, user_input: &str) -> Admitted {
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
            source_binding_id: Some("draft-source".into()),
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
    coordinator
        .submit_turn(SubmitTurnRequest {
            scope: turn_scope.clone(),
            actor: TurnActor::new(actor),
            accepted_message_ref: AcceptedMessageRef::new(format!("msg:{}", accepted.message_id))
                .unwrap(),
            source_binding_ref: SourceBindingRef::new("draft-source").unwrap(),
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
    let context = LoopRunContext::new(
        turn_scope,
        claimed.state.turn_id,
        claimed.state.run_id,
        claimed.resolved_run_profile.clone(),
    )
    .with_actor(claimed.state.actor.clone().unwrap())
    .with_accepted_message_ref(claimed.state.accepted_message_ref.clone());
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
    admission.check_and_start().await.unwrap();
    Admitted {
        input: json!({"conversation_id":conversation,"message_id":accepted.message_id,
        "turn_id":context.turn_id,"run_id":context.run_id,"user_input":user_input,"history":[]}),
        admission,
        context,
        state,
    }
}
