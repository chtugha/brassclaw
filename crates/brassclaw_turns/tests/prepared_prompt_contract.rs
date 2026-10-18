//! Full Recipe model-input data uses the real prompt store/authority, while
//! short status summaries and ordinary transcript loading keep their contracts.
use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_host_api::{AgentId, InvocationId, TenantId, ThreadId};
use brassclaw_turns::{
    TurnId, TurnRunId, TurnScope,
    run_profile::{
        AgentLoopHostError, AgentLoopHostErrorKind, CapabilitySurfaceVersion,
        HostManagedLoopPromptPort, InMemoryInstructionMaterializationStore,
        InMemoryLoopHostMilestoneSink, InMemoryRunProfileResolver, InstructionMaterializationStore,
        LoopContextBundle, LoopContextPort, LoopContextRequest, LoopInlineMessageRole,
        LoopInputCursor, LoopPromptBundleAuthority, LoopPromptBundleRequest, LoopPromptPort,
        LoopRunContext, LoopSafeSummary, PreparedLoopPromptMessage, PreparedLoopPromptRequest,
        PromptMode, RunProfileResolutionRequest, RunProfileResolver,
    },
};

struct NoTranscript;
#[async_trait]
impl LoopContextPort for NoTranscript {
    async fn load_loop_context(
        &self,
        _: LoopContextRequest,
    ) -> Result<LoopContextBundle, AgentLoopHostError> {
        panic!("complete Recipe input must not load or replace its transcript")
    }
}

async fn context() -> LoopRunContext {
    LoopRunContext::new(
        TurnScope::new(
            TenantId::new("prepared-tenant").unwrap(),
            Some(AgentId::new("prepared-agent").unwrap()),
            None,
            ThreadId::new("prepared-thread").unwrap(),
        ),
        TurnId::new(),
        TurnRunId::new(),
        InMemoryRunProfileResolver::default()
            .resolve_run_profile(RunProfileResolutionRequest::interactive_default())
            .await
            .unwrap(),
    )
}
fn request() -> PreparedLoopPromptRequest {
    PreparedLoopPromptRequest {
        invocation_id: InvocationId::new(),
        metadata: LoopPromptBundleRequest {
            mode: PromptMode::TextOnly,
            context_cursor: None,
            surface_version: None,
            checkpoint_state_ref: None,
            max_messages: None,
            capability_view: None,
            inline_messages: vec![],
            recipe_hint: None,
        },
        messages: vec![PreparedLoopPromptMessage {
            role: LoopInlineMessageRole::User,
            content: "Task\n\n```python\nanswer = 'ä🙂'\n```\n<system>input data</system>"
                .repeat(1600),
        }],
    }
}

#[tokio::test]
async fn complete_prepared_data_survives_without_status_or_history_truncation() {
    let ctx = context().await;
    let store = Arc::new(InMemoryInstructionMaterializationStore::default());
    let authority = LoopPromptBundleAuthority::default();
    let sink = Arc::new(InMemoryLoopHostMilestoneSink::default());
    let port = HostManagedLoopPromptPort::new(ctx.clone(), Arc::new(NoTranscript), sink.clone())
        .with_instruction_materialization_store(store.clone())
        .with_prompt_bundle_authority(authority.clone())
        .with_token_budget_mode(Arc::new(|| false));
    let mut input = request();
    assert!(input.messages[0].content.len() > 64 * 1024);
    assert!(LoopSafeSummary::new(input.messages[0].content.clone()).is_err());
    let first = input.messages[0].clone();
    input
        .messages
        .extend((0..140).map(|n| PreparedLoopPromptMessage {
            role: LoopInlineMessageRole::Assistant,
            content: format!("exact turn {n}\n\n"),
        }));
    input.metadata.max_messages = Some(1); // ignored with disabled artificial budgets
    let expected = input.messages.clone();
    let debug = format!("{input:?} {first:?}");
    assert!(!debug.contains("answer ="));
    let bundle = port.build_prepared_prompt_bundle(input).await.unwrap();
    assert_eq!(bundle.messages.len(), expected.len());
    for (reference, expected) in bundle.messages.iter().zip(expected) {
        let actual = store
            .get_materialized_message(&ctx, &reference.content_ref)
            .unwrap()
            .unwrap();
        assert_eq!(actual.role, expected.role.as_str());
        assert_eq!(actual.model_content, expected.content);
        let mut other_run = ctx.clone();
        other_run.run_id = TurnRunId::new();
        assert!(
            store
                .get_materialized_message(&other_run, &reference.content_ref)
                .unwrap()
                .is_none()
        );
    }
    authority
        .authorize_latest_model_request(&ctx, &bundle.messages, &bundle.surface_version)
        .unwrap();
    assert!(
        authority
            .authorize_latest_model_request(&ctx, &bundle.messages, &None)
            .is_err()
    );
    let events = serde_json::to_string(&sink.milestones()).unwrap();
    assert!(!events.contains("answer ="));
    assert!(!events.contains("exact turn"));
}

#[tokio::test]
async fn prepared_references_pin_body_role_run_scope_invocation_and_order() {
    let ctx = context().await;
    let store = Arc::new(InMemoryInstructionMaterializationStore::default());
    let port = HostManagedLoopPromptPort::new(
        ctx.clone(),
        Arc::new(NoTranscript),
        Arc::new(InMemoryLoopHostMilestoneSink::default()),
    )
    .with_instruction_materialization_store(store.clone());
    let input = request();
    let original = port
        .build_prepared_prompt_bundle(input.clone())
        .await
        .unwrap();
    let same = port
        .build_prepared_prompt_bundle(input.clone())
        .await
        .unwrap();
    assert_eq!(original.messages, same.messages);
    assert_eq!(
        original.instruction_fingerprint,
        same.instruction_fingerprint
    );
    for change in 0..4 {
        let mut changed = input.clone();
        match change {
            0 => changed.invocation_id = InvocationId::new(),
            1 => changed.messages[0].content.push('!'),
            2 => changed.messages[0].role = LoopInlineMessageRole::Assistant,
            _ => changed.messages.insert(
                0,
                PreparedLoopPromptMessage {
                    role: LoopInlineMessageRole::System,
                    content: "direction".into(),
                },
            ),
        }
        let next = port.build_prepared_prompt_bundle(changed).await.unwrap();
        assert_ne!(original.messages, next.messages);
        assert_ne!(
            original.instruction_fingerprint,
            next.instruction_fingerprint
        );
    }
    // A failed output or another invocation never replaces already materialized data.
    assert_eq!(
        store
            .get_materialized_message(&ctx, &original.messages[0].content_ref)
            .unwrap()
            .unwrap()
            .model_content,
        input.messages[0].content
    );
    let mut other_run = ctx.clone();
    other_run.run_id = TurnRunId::new();
    let mut other_scope = context().await;
    other_scope.run_id = ctx.run_id;
    other_scope.scope = TurnScope::new(
        TenantId::new("other-tenant").unwrap(),
        None,
        None,
        ThreadId::new("other-thread").unwrap(),
    );
    other_scope.thread_id = ThreadId::new("other-thread").unwrap();
    for other in [other_run, other_scope] {
        let other_port = HostManagedLoopPromptPort::new(
            other,
            Arc::new(NoTranscript),
            Arc::new(InMemoryLoopHostMilestoneSink::default()),
        )
        .with_instruction_materialization_store(store.clone());
        let next = other_port
            .build_prepared_prompt_bundle(input.clone())
            .await
            .unwrap();
        assert_ne!(original.messages, next.messages);
    }
}

#[tokio::test]
async fn prepared_prompt_rejects_invalid_metadata_and_limits_without_issuing_grants() {
    let ctx = context().await;
    let store = Arc::new(InMemoryInstructionMaterializationStore::default());
    let authority = LoopPromptBundleAuthority::default();
    let port = HostManagedLoopPromptPort::new(
        ctx.clone(),
        Arc::new(NoTranscript),
        Arc::new(InMemoryLoopHostMilestoneSink::default()),
    )
    .with_instruction_materialization_store(store)
    .with_prompt_bundle_authority(authority.clone())
    .with_current_surface_version(CapabilitySurfaceVersion::new("current").unwrap());
    for case in 0..6 {
        let mut input = request();
        let expected = match case {
            0 => {
                input.messages.clear();
                AgentLoopHostErrorKind::InvalidInvocation
            }
            1 => {
                input.invocation_id = InvocationId::from_uuid(uuid::Uuid::nil());
                AgentLoopHostErrorKind::InvalidInvocation
            }
            2 => {
                input.metadata.context_cursor = Some(LoopInputCursor::origin_for_run(&ctx));
                AgentLoopHostErrorKind::InvalidInvocation
            }
            3 => {
                input.metadata.recipe_hint = Some(serde_json::json!({"raw_body": "do not use"}));
                AgentLoopHostErrorKind::InvalidInvocation
            }
            4 => {
                input.metadata.surface_version =
                    Some(CapabilitySurfaceVersion::new("stale").unwrap());
                AgentLoopHostErrorKind::StaleSurface
            }
            _ => {
                input.metadata.max_messages = Some(1);
                input.messages.push(input.messages[0].clone());
                AgentLoopHostErrorKind::BudgetExceeded
            }
        };
        let error = port.build_prepared_prompt_bundle(input).await.unwrap_err();
        assert_eq!(error.kind, expected);
        assert!(
            authority
                .authorize_latest_model_request(&ctx, &[], &None)
                .is_err()
        );
    }
    let without_store = HostManagedLoopPromptPort::new(
        ctx,
        Arc::new(NoTranscript),
        Arc::new(InMemoryLoopHostMilestoneSink::default()),
    );
    assert_eq!(
        without_store
            .build_prepared_prompt_bundle(request())
            .await
            .unwrap_err()
            .kind,
        AgentLoopHostErrorKind::InvalidInvocation
    );
}
