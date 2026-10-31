//! Actual task host, PostgreSQL selection and guarded SDK lifecycle.
use super::*;
use brassclaw_reborn::{
    loop_driver_host::{
        RebornLoopDriverHostFactory, RebornLoopDriverHostRequest, TextOnlyLoopHostConfig,
    },
    model_gateway::{LlmModelProfilePolicy, LlmProviderModelGateway},
};
use brassclaw_turns::run_profile::{AgentLoopDriverRunRequest, MontyTaskAttempt, MontyTaskHandoff};
use serde_json::{Value, json};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_task_memory_port_checks_scope_cancellation_and_weak_lifetime() {
    brassclaw_reborn::loop_driver_host::init_compaction_summarizer(
        include_str!("../../brassclaw_loop_support/prompts/compaction_summarizer_fresh.md").into(),
    );
    #[derive(Default)]
    struct Endpoint {
        started: tokio::sync::Notify,
        release: tokio::sync::Notify,
        calls: std::sync::atomic::AtomicUsize,
    }
    async fn embedding(
        axum::extract::State(state): axum::extract::State<Arc<Endpoint>>,
        axum::Json(body): axum::Json<Value>,
    ) -> axum::Json<Value> {
        state
            .calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        state.started.notify_one();
        state.release.notified().await;
        axum::Json(
            json!({"data":body["input"].as_array().unwrap().iter().enumerate()
            .map(|(index,_)|json!({"index":index,"embedding":[1.0,0.0,0.0]})).collect::<Vec<_>>()}),
        )
    }
    let endpoint = Arc::new(Endpoint::default());
    let (address, stop, server) = crate::test_loopback_http::spawn(
        axum::Router::new()
            .route("/v1/embeddings", axum::routing::post(embedding))
            .with_state(endpoint.clone()),
    )
    .await;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let (admitted, claimed) = crate::native_admission_fixture::reserve_unstarted_with_claim(
        rig.pool.clone(),
        "memory task lifetime",
    )
    .await;
    let context = admitted.context.clone();
    let tenant = context.scope.tenant_id.as_str();
    crate::PgProviderRepo::new(rig.pool.as_ref().clone(),tenant).replace_revision(serde_json::from_value(json!({
        "id":"task-lifetime","protocol":"open_ai_completions","default_base_url":format!("http://{address}/v1"),
        "model_env":"BRASSCLAW_TASK_LIFETIME_TEST_MODEL","default_model":"space","api_key_required":false,
        "description":"Task memory lifetime fixture","request_policy":brassclaw_llm::ProviderRequestPolicy {max_retries:0,..Default::default()},
        "embedding_models":{"space":{"dimension":3,"cache_max_entries":8}}
    })).unwrap(),None).await.unwrap();
    crate::db_config::save_config_changes(
        &rig.pool,
        tenant,
        &[
            ("embedding.provider_id", Some("task-lifetime")),
            ("embedding.model", Some("space")),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            "task-memory-lifetime-master-fixture".into(),
            tenant,
        )
        .unwrap(),
    ));
    let sources = crate::task_model_provider::TaskProviderConfiguration::new(
        rig.pool.as_ref().clone(),
        tenant,
        keys,
    )
    .with_session(Arc::new(brassclaw_llm::SessionManager::new(
        Default::default(),
    )));
    let gateway = Arc::new(
        LlmProviderModelGateway::new(
            Arc::new(crate::runtime::PlaceholderLlmProvider),
            LlmModelProfilePolicy::new()
                .allow_model_profile(context.resolved_run_profile.model_profile_id.clone(), None),
        )
        .with_task_source(sources.role(brassclaw_llm::ProviderRole::Kohai)),
    );
    let factory = RebornLoopDriverHostFactory::new(
        Arc::new(brassclaw_threads::PgSessionThreadService::new(
            rig.pool.clone(),
            tenant,
        )),
        brassclaw_threads::ThreadScope {
            tenant_id: context.scope.tenant_id.clone(),
            agent_id: context.scope.agent_id.clone().unwrap(),
            project_id: context.scope.project_id.clone(),
            owner_user_id: None,
        },
        gateway,
        Arc::new(brassclaw_turns::InMemoryCheckpointStateStore::default()),
        admitted.state,
        Arc::new(brassclaw_turns::InMemoryLoopCheckpointStore::default()),
        Arc::new(brassclaw_turns::run_profile::InMemoryLoopHostMilestoneSink::default()),
        TextOnlyLoopHostConfig {
            max_messages: 8,
            require_model_route_snapshot: false,
        },
        brassclaw_turns::run_profile::InstructionSafetyContext::local_development_noop(),
    );
    let host = Arc::new(MontyTaskHost::new(
        MontyTaskHandoff::new(
            AgentLoopDriverRunRequest {
                turn_id: context.turn_id,
                run_id: context.run_id,
                resolved_run_profile: context.resolved_run_profile.clone(),
            },
            MontyTaskAttempt {
                run_id: context.run_id,
                runner_id: claimed.runner_id,
                lease_token: claimed.lease_token,
            },
            Arc::new(
                factory
                    .build_text_only_host(RebornLoopDriverHostRequest {
                        claimed_run: claimed,
                        loop_run_context: context.clone(),
                    })
                    .await
                    .unwrap(),
            ),
        )
        .unwrap(),
    ));
    let captured = host
        .capture_task_model_selection(None, None)
        .await
        .unwrap()
        .unwrap();
    admitted
        .admission
        .bind_task_models(Some(captured.retained_metadata().unwrap()))
        .unwrap();
    admitted.admission.check_and_start().await.unwrap();
    drop(captured);
    bind(&host).unwrap();
    assert!(bind(&host).is_err(), "a task dependency cannot be rebound");
    let port = TaskMemoryEmbeddings {
        host: Arc::downgrade(&host),
        provider: OnceLock::new(),
    };
    let scope = ResourceScope {
        tenant_id: context.scope.tenant_id.clone(),
        user_id: context.actor.as_ref().unwrap().user_id.clone(),
        agent_id: context.scope.agent_id.clone(),
        project_id: context.scope.project_id.clone(),
        thread_id: Some(context.thread_id.clone()),
        invocation_id: brassclaw_host_api::InvocationId::new(),
    };
    for field in 0..5 {
        let mut foreign = scope.clone();
        match field {
            0 => foreign.tenant_id = brassclaw_host_api::TenantId::new("foreign").unwrap(),
            1 => foreign.user_id = brassclaw_host_api::UserId::new("foreign").unwrap(),
            2 => foreign.agent_id = None,
            3 => foreign.project_id = None,
            _ => foreign.thread_id = None,
        }
        assert!(port.snapshot(&foreign).await.is_err());
    }
    let provider = port.snapshot(&scope).await.unwrap().unwrap();
    assert!(Arc::ptr_eq(
        &provider,
        &port.snapshot(&scope).await.unwrap().unwrap()
    ));
    let in_flight = provider.clone();
    let pending = tokio::spawn(async move { in_flight.embed("late cancelled embedding").await });
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        endpoint.started.notified(),
    )
    .await
    .unwrap();
    host.fence_dispatch();
    endpoint.release.notify_one();
    assert!(
        pending.await.unwrap().is_err(),
        "late response cannot resume a fenced attempt"
    );
    assert!(port.snapshot(&scope).await.is_err());
    assert!(provider.embed("no second dispatch").await.is_err());
    let weak = Arc::downgrade(&host);
    drop(host);
    assert!(
        weak.upgrade().is_none(),
        "retained native provider/port cannot keep the task alive"
    );
    assert!(provider.embed("completed task").await.is_err());
    assert_eq!(endpoint.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    stop.cancel();
    server.await.unwrap();
}
