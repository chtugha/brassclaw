//! Actual PostgreSQL goal storage: complete data, uniqueness and explicit cleanup.
use brassclaw_host_api::{TenantId, ThreadId};
use brassclaw_reborn::subagent::goal_store::{
    PgSubagentGoalStore, SubagentGoal, SubagentGoalStore, SubagentGoalStoreError,
};
use brassclaw_turns::{TurnRunId, TurnScope};

#[tokio::test]
async fn native_subagent_goal_retains_complete_large_data_and_explicit_deletion() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let store = PgSubagentGoalStore::new((*rig.pool).clone());
    let scope = TurnScope::new(
        TenantId::new("goal-storage-owner").unwrap(),
        None,
        None,
        ThreadId::new("goal-storage-thread").unwrap(),
    );
    let run = TurnRunId::new();
    let expected = SubagentGoal {
        task: "ü\n\"\\{{vars.source}}".repeat(8000),
        handoff: Some("complete handoff 🦀\n".repeat(8000)),
    };
    assert!(expected.task.len() > 64 * 1024);
    assert!(expected.handoff.as_ref().unwrap().len() > 64 * 1024);
    store.put_goal(&scope, run, expected.clone()).await.unwrap();
    assert_eq!(store.get_goal(&scope, run).await.unwrap(), expected);
    assert!(matches!(
        store.put_goal(&scope, run, SubagentGoal { task: "replacement".into(), handoff: None }).await,
        Err(SubagentGoalStoreError::DuplicateKey { run_id }) if run_id == run
    ));
    assert_eq!(store.get_goal(&scope, run).await.unwrap(), expected);
    // A second native store sees the exact committed data, not a local cache.
    let reopened = PgSubagentGoalStore::new((*rig.pool).clone());
    assert_eq!(reopened.get_goal(&scope, run).await.unwrap(), expected);
    let foreign = TurnScope::new(
        TenantId::new("goal-storage-other").unwrap(),
        None,
        None,
        scope.thread_id.clone(),
    );
    assert!(matches!(
        reopened.get_goal(&foreign, run).await,
        Err(SubagentGoalStoreError::NotFound { .. })
    ));
    reopened.delete_goal(&foreign, run).await.unwrap();
    assert_eq!(reopened.get_goal(&scope, run).await.unwrap(), expected);
    reopened.delete_goal(&scope, run).await.unwrap();
    reopened.delete_goal(&scope, run).await.unwrap();
    assert!(matches!(
        store.get_goal(&scope, run).await,
        Err(SubagentGoalStoreError::NotFound { .. })
    ));
    let row_count: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM brassclaw_subagent_goals", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(row_count, 0);
}

/// Consume committed goals through the actual source, transcript adapter and
/// prompt authority. This is host handoff acceptance, not R09 activation.
#[tokio::test]
async fn native_subagent_prompt_preserves_large_goal_and_complete_eligible_context() {
    use brassclaw_host_api::{AgentId, CapabilityId, RuntimeKind};
    use brassclaw_loop_support::{
        SubagentLoopPromptPort, SubagentPromptComposer, ThreadBackedLoopContextPort,
    };
    use brassclaw_reborn::subagent::{
        gate_resolution::BoundedSubagentGateResolutionStore,
        prompt_material::GateBackedSubagentPromptMaterialSource,
    };
    use brassclaw_threads::{
        AcceptInboundMessageRequest, EnsureThreadRequest, MessageContent, PgSessionThreadService,
        SessionThreadService, ThreadScope,
    };
    use brassclaw_turns::{
        TurnId,
        run_profile::{
            CapabilityDescriptorView, CapabilitySurfaceVersion, ConcurrencyHint,
            HostManagedLoopPromptPort, InMemoryInstructionMaterializationStore,
            InMemoryLoopHostMilestoneSink, InMemoryRunProfileResolver,
            InstructionMaterializationStore, LoopModelCapabilityView, LoopPromptBundleAuthority,
            LoopPromptBundleRequest, LoopPromptPort, LoopRunContext, PromptMode,
            RunProfileResolutionRequest, RunProfileResolver, VisibleCapabilitySurface,
        },
    };
    use std::sync::Arc;
    let rig = crate::runtime::test_pg::pg_rig().await;
    // Exercise the production component boot: verified DB-backed directions,
    // not an artificial test-local prompt or a relaxed initialization gate.
    let booted = crate::booted_db::BootedDb::from_migrated_pool(Arc::clone(&rig.pool));
    crate::component_boot::initialize_runtime_components(&booted, "subagent-prompt-owner")
        .await
        .unwrap();
    let tenant = TenantId::new("subagent-prompt-owner").unwrap();
    let agent = AgentId::new("subagent-prompt-agent").unwrap();
    let thread_id = ThreadId::new("subagent-prompt-thread").unwrap();
    let scope = TurnScope::new(tenant.clone(), Some(agent.clone()), None, thread_id.clone());
    let context = LoopRunContext::new(
        scope,
        TurnId::new(),
        TurnRunId::new(),
        InMemoryRunProfileResolver::default()
            .resolve_run_profile(RunProfileResolutionRequest::interactive_default())
            .await
            .unwrap(),
    );
    let store = Arc::new(PgSubagentGoalStore::new((*rig.pool).clone()));
    let goal = SubagentGoal {
        task: "ü🙂  task\n\n```python\nx = '{{vars.source}}'\n```".repeat(9000),
        handoff: Some("Exact handoff\twith  spaces\n\n".repeat(9000)),
    };
    store
        .put_goal(&context.scope, context.run_id, goal.clone())
        .await
        .unwrap();
    let thread_scope = ThreadScope {
        tenant_id: tenant.clone(),
        agent_id: agent,
        project_id: None,
        owner_user_id: None,
    };
    let threads = Arc::new(PgSessionThreadService::new(
        Arc::clone(&rig.pool),
        tenant.as_str(),
    ));
    threads
        .ensure_thread(EnsureThreadRequest {
            scope: thread_scope.clone(),
            thread_id: Some(thread_id.clone()),
            created_by_actor_id: "operator".into(),
            title: None,
            metadata_json: Some(
                serde_json::to_string(&brassclaw_loop_support::SubagentThreadMetadata {
                    kind: brassclaw_loop_support::SubagentThreadKind::Subagent,
                    parent_run_id: TurnRunId::new(),
                    parent_thread_id: ThreadId::new("parent-prompt-thread").unwrap(),
                    tree_root_run_id: context.run_id,
                    child_run_id: context.run_id,
                    subagent_kind: brassclaw_loop_support::SubagentKindId::new("researcher")
                        .unwrap(),
                    mode: brassclaw_loop_support::SpawnSubagentMode::Blocking,
                    result_ref: brassclaw_turns::LoopResultRef::new("result:child-prompt").unwrap(),
                    handoff: None,
                })
                .unwrap(),
            ),
        })
        .await
        .unwrap();
    for index in 0..142 {
        let accepted = threads
            .accept_inbound_message(AcceptInboundMessageRequest {
                scope: thread_scope.clone(),
                thread_id: thread_id.clone(),
                actor_id: "operator".into(),
                source_binding_id: None,
                reply_target_binding_id: None,
                external_event_id: None,
                content: MessageContent::text(format!("child-history-{index}")),
            })
            .await
            .unwrap();
        threads
            .mark_message_submitted(
                &thread_scope,
                &thread_id,
                accepted.message_id,
                context.turn_id.to_string(),
                context.run_id.to_string(),
            )
            .await
            .unwrap();
    }
    let capability_ids = [
        brassclaw_host_runtime::HTTP_CAPABILITY_ID,
        brassclaw_loop_support::DEFAULT_SPAWN_SUBAGENT_CAPABILITY_ID,
    ];
    let surface = VisibleCapabilitySurface {
        version: CapabilitySurfaceVersion::new("subagent-prompt-surface").unwrap(),
        descriptors: capability_ids
            .into_iter()
            .map(|id| CapabilityDescriptorView {
                capability_id: CapabilityId::new(id).unwrap(),
                provider: None,
                runtime: RuntimeKind::FirstParty,
                safe_name: id.into(),
                safe_description: "Operation".into(),
                concurrency_hint: ConcurrencyHint::Exclusive,
                parameters_schema: serde_json::json!({"type":"object"}),
            })
            .collect(),
    };
    let materialized = Arc::new(InMemoryInstructionMaterializationStore::default());
    let authority = LoopPromptBundleAuthority::default();
    let transcript =
        ThreadBackedLoopContextPort::new(threads.clone(), thread_scope, context.clone(), 1)
            .with_token_budget_mode(Arc::new(|| false));
    let inner = HostManagedLoopPromptPort::new(
        context.clone(),
        Arc::new(transcript),
        Arc::new(InMemoryLoopHostMilestoneSink::default()),
    )
    .with_token_budget_mode(Arc::new(|| false))
    .with_instruction_materialization_store(materialized.clone())
    .with_prompt_bundle_authority(authority.clone())
    .with_current_surface(surface.clone());
    let source = GateBackedSubagentPromptMaterialSource::new(
        store.clone(),
        Arc::new(BoundedSubagentGateResolutionStore::new()),
        threads,
    );
    let port = SubagentLoopPromptPort::new(
        Arc::new(inner),
        context.clone(),
        SubagentPromptComposer::new(Arc::new(source)),
    );
    let bundle = port
        .build_prompt_bundle(LoopPromptBundleRequest {
            mode: PromptMode::TextOnly,
            context_cursor: None,
            surface_version: Some(surface.version),
            capability_view: Some(LoopModelCapabilityView {
                visible_capability_ids: capability_ids
                    .into_iter()
                    .map(|id| CapabilityId::new(id).unwrap())
                    .collect(),
            }),
            checkpoint_state_ref: None,
            max_messages: Some(1),
            inline_messages: vec![],
            recipe_hint: None,
        })
        .await
        .unwrap();
    let expected_goal = format!(
        "Subagent task:\n{}\n\nSubagent handoff:\n{}",
        goal.task,
        goal.handoff.as_deref().unwrap()
    );
    let mut full_goal_count = 0;
    let mut history_count = 0;
    let mut definitions = String::new();
    for message in &bundle.messages {
        if let Some(body) = materialized
            .get_materialized_message(&context, &message.content_ref)
            .unwrap()
        {
            if body.model_content == expected_goal {
                full_goal_count += 1;
            }
            if body.role == "system" {
                definitions.push_str(&body.model_content);
            }
        } else {
            // Durable transcript references remain untouched and resolvable.
            history_count += 1;
        }
    }
    assert_eq!(full_goal_count, 1);
    assert_eq!(history_count, 142);
    assert!(definitions.contains(brassclaw_host_runtime::HTTP_CAPABILITY_ID));
    assert!(!definitions.contains(brassclaw_loop_support::DEFAULT_SPAWN_SUBAGENT_CAPABILITY_ID));
    authority
        .authorize_latest_model_request(&context, &bundle.messages, &bundle.surface_version)
        .unwrap();
    assert!(
        authority
            .authorize_latest_model_request(&context, &bundle.messages, &bundle.surface_version)
            .is_err()
    );
    assert_eq!(
        store
            .get_goal(&context.scope, context.run_id)
            .await
            .unwrap(),
        goal
    );
    store
        .delete_goal(&context.scope, context.run_id)
        .await
        .unwrap();
}
