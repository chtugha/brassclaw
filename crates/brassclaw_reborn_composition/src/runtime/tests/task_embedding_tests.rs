//! Ordinary No-Match Monty -> model-requested Memory Tool -> native kernel ->
//! actual guarded embedding SDK. Controlled HTTP verifies custody, not quality.
use super::*;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Default)]
struct Endpoint {
    round: AtomicUsize,
    scenario: AtomicUsize,
    hold: AtomicBool,
    started: tokio::sync::Notify,
    release: tokio::sync::Notify,
    embeddings: tokio::sync::Mutex<Vec<Value>>,
}

async fn model(
    axum::extract::State(state): axum::extract::State<Arc<Endpoint>>,
    axum::Json(body): axum::Json<Value>,
) -> axum::Json<Value> {
    let round = state.round.fetch_add(1, Ordering::SeqCst);
    if state.hold.swap(false, Ordering::SeqCst) {
        state.started.notify_one();
        state.release.notified().await;
    }
    let (message, finish) = if round == 0 {
        let functions = body["tools"].as_array().expect("ordinary tool surface");
        let name = functions
            .iter()
            .filter_map(|tool| tool["function"]["name"].as_str())
            .find(|name| name.contains("memory_write"))
            .expect("real registered memory write advertised");
        (
            json!({"role":"assistant","content":null,"tool_calls":[{"id":format!("memory-call-{}",state.scenario.load(Ordering::SeqCst)),"type":"function","function":{"name":name,"arguments":json!({"target":format!("notes/task-selection-{}.md",state.scenario.load(Ordering::SeqCst)),"content":"retained task embedding probe","append":false}).to_string()}}]}),
            "tool_calls",
        )
    } else {
        (
            json!({"role":"assistant","content":"memory handoff observed"}),
            "stop",
        )
    };
    axum::Json(
        json!({"id":"task-embedding-model","object":"chat.completion","created":1,"model":body["model"],
        "choices":[{"index":0,"message":message,"finish_reason":finish}],
        "usage":{"prompt_tokens":8,"completion_tokens":4,"total_tokens":12}}),
    )
}

async fn embedding(
    axum::extract::State(state): axum::extract::State<Arc<Endpoint>>,
    axum::Json(body): axum::Json<Value>,
) -> axum::Json<Value> {
    state.embeddings.lock().await.push(body.clone());
    let vector = if body["model"] == "space-a" {
        vec![1.0, 0.0, 0.0]
    } else {
        vec![0.0, 1.0, 0.0]
    };
    axum::Json(
        json!({"data":body["input"].as_array().unwrap().iter().enumerate()
        .map(|(index,_)| json!({"index":index,"embedding":vector})).collect::<Vec<_>>()}),
    )
}

async fn select(pool: &brassclaw_pg::PgPool, tenant: &str, model: Option<&str>) {
    crate::db_config::save_config_changes(
        pool,
        tenant,
        &[
            ("embedding.provider_id", model.map(|_| "task-memory")),
            ("embedding.model", model),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
}

// Exercise the retained native entry as well as the model-requested generic
// entry. This is a read-only transport qualification, not Recipe activation.
async fn retained_search(
    runtime: &crate::RebornRuntime,
    host: &Arc<brassclaw_reborn::monty_task_host::MontyTaskHost>,
) {
    use brassclaw_authorization::{
        InstanceToolRule, LiveStableToolPolicy, StableToolPolicySnapshot, ToolExecutionRules,
    };
    use brassclaw_host_api::{
        CapabilityId, CapabilitySet, ExecutionContext, ExtensionId, MountAlias, MountGrant,
        MountPermissions, MountView, NetworkPolicy, RuntimeKind, TrustClass, VirtualPath,
    };
    use brassclaw_host_runtime::{
        MEMORY_SEARCH_CAPABILITY_ID, RuntimeCapabilityOutcome, RuntimeCapabilityRequest,
    };
    let mounts = MountView::new(vec![MountGrant::new(
        MountAlias::new("/memory").unwrap(),
        VirtualPath::new("/memory").unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    let capability = CapabilityId::new(MEMORY_SEARCH_CAPABILITY_ID).unwrap();
    let tool = uuid::Uuid::new_v4();
    let policy = Arc::new(
        LiveStableToolPolicy::new(StableToolPolicySnapshot {
            revision: 1,
            tools: [(
                tool,
                InstanceToolRule {
                    enabled: true,
                    revision: 1,
                    execution: ToolExecutionRules {
                        allowed_effects: vec![
                            EffectKind::ReadFilesystem,
                            EffectKind::WriteFilesystem,
                        ],
                        mounts: mounts.clone(),
                        network: NetworkPolicy::default(),
                        secrets: vec![],
                        resource_ceiling: None,
                    },
                },
            )]
            .into(),
            capabilities: [(capability.clone(), tool)].into(),
        })
        .unwrap(),
    );
    let kernel = runtime.services.monty_kernel.as_ref().unwrap();
    let original = kernel.builtin(&capability, None).unwrap();
    let native = kernel
        .builtin_with_policy(&capability, original.native_identity().unwrap(), policy)
        .unwrap();
    native.verify_native_artifact().await.unwrap();
    let task = host.run_context();
    let mut execution = ExecutionContext::local_default(
        task.actor.as_ref().unwrap().user_id.clone(),
        ExtensionId::new("builtin").unwrap(),
        RuntimeKind::FirstParty,
        TrustClass::FirstParty,
        CapabilitySet::default(),
        mounts,
    )
    .unwrap();
    execution.tenant_id = task.scope.tenant_id.clone();
    execution.agent_id = task.scope.agent_id.clone();
    execution.project_id = task.scope.project_id.clone();
    execution.thread_id = Some(task.thread_id.clone());
    execution.resource_scope.tenant_id = execution.tenant_id.clone();
    execution.resource_scope.agent_id = execution.agent_id.clone();
    execution.resource_scope.project_id = execution.project_id.clone();
    execution.resource_scope.thread_id = execution.thread_id.clone();
    let result = host
        .invoke_retained_tool(
            &native,
            RuntimeCapabilityRequest::new(
                execution,
                capability,
                ResourceEstimate::default(),
                json!({"query":"retained native original-task vector probe","limit":5}),
                brassclaw_trust::TrustDecision {
                    effective_trust: brassclaw_trust::EffectiveTrustClass::user_trusted(),
                    authority_ceiling: brassclaw_trust::AuthorityCeiling {
                        allowed_effects: vec![
                            EffectKind::ReadFilesystem,
                            EffectKind::WriteFilesystem,
                        ],
                        max_resource_ceiling: None,
                    },
                    provenance: brassclaw_trust::TrustProvenance::AdminConfig,
                    evaluated_at: chrono::Utc::now(),
                },
            ),
        )
        .await
        .unwrap();
    assert!(matches!(result, RuntimeCapabilityOutcome::Completed(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_global_no_match_memory_retains_task_embedding_before_first_use() {
    let state = Arc::new(Endpoint::default());
    let (address, stop, server) = crate::test_loopback_http::spawn(
        axum::Router::new()
            .route("/v1/chat/completions", axum::routing::post(model))
            .route("/v1/embeddings", axum::routing::post(embedding))
            .with_state(state.clone()),
    )
    .await;
    let rig = crate::runtime::test_pg::pg_rig().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let tenant = "task-memory-native";
    let home = tempfile::tempdir().unwrap();
    let repo = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), tenant);
    repo.replace_revision(serde_json::from_value(json!({"id":"task-memory","protocol":"open_ai_completions",
        "default_base_url":format!("http://{address}/v1"),"api_key_required":false,
        "default_model":"task-model","model_env":"BRASSCLAW_TASK_MEMORY_TEST_MODEL",
        "description":"Task memory transport fixture","request_policy":brassclaw_llm::ProviderRequestPolicy {max_retries:0, ..Default::default()},
        "embedding_models":{"space-a":{"dimension":3,"cache_max_entries":8},"space-b":{"dimension":3,"cache_max_entries":8}}
    })).unwrap(), None).await.unwrap();
    crate::db_config::save_config_changes(
        &rig.pool,
        tenant,
        &[
            ("llm.default.provider_id", Some("task-memory")),
            ("llm.default.model", Some("task-model")),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    select(&rig.pool, tenant, Some("space-a")).await;
    let runtime = Arc::new(
        Box::pin(build_reborn_runtime(
            RebornRuntimeInput::from_services(
                rig.build_input("task-memory-owner", home.path())
                    .with_runtime_policy(local_dev_runtime_policy()),
            )
            .with_identity(RebornRuntimeIdentity {
                tenant_id: tenant.into(),
                ..RebornRuntimeIdentity::reborn_cli()
            }),
        ))
        .await
        .unwrap(),
    );
    let root = runtime.global_monty_owner.client().root_identity();
    for (scenario, before, after, expected) in [
        (0, Some("space-a"), Some("space-b"), Some("space-a")),
        (1, Some("space-b"), Some("space-a"), Some("space-b")),
        (2, None, Some("space-a"), None),
        (3, Some("not-a-declared-space"), Some("space-a"), None),
    ] {
        select(&rig.pool, tenant, before).await;
        state.scenario.store(scenario, Ordering::SeqCst);
        state.round.store(0, Ordering::SeqCst);
        state.hold.store(true, Ordering::SeqCst);
        let conversation = runtime.new_conversation().await.unwrap();
        let thread = conversation.0.clone();
        let task_runtime = runtime.clone();
        let task = tokio::spawn(async move {
            task_runtime
                .send_user_message(
                    &conversation,
                    "opaque ordinary task embedding selection probe",
                )
                .await
        });
        tokio::time::timeout(Duration::from_secs(10), state.started.notified())
            .await
            .expect("actual model request after original task selection");
        select(&rig.pool, tenant, after).await;
        let count = state.embeddings.lock().await.len();
        if scenario < 2 {
            let host = runtime
                .monty_test_controls
                .1
                .retained_host_for_test(&thread)
                .unwrap()
                .unwrap();
            retained_search(&runtime, &host).await;
        }
        state.release.notify_one();
        let outcome = task.await.unwrap().unwrap();
        if scenario != 3 {
            assert_eq!(
                outcome.status,
                TurnStatus::Completed,
                "scenario {scenario}: {outcome:?}"
            );
            assert_eq!(outcome.text.as_deref(), Some("memory handoff observed"));
        } else {
            assert_eq!(
                outcome.status,
                TurnStatus::Failed,
                "invalid original selection remains a failed Tool, without another model round"
            );
            assert_eq!(state.round.load(Ordering::SeqCst), 1);
        }
        let calls = state.embeddings.lock().await;
        if let Some(expected) = expected {
            assert!(
                calls.len() > count,
                "actual embedding request in scenario {scenario}"
            );
            assert!(calls[count..].iter().all(|call| call["model"] == expected));
        } else {
            assert_eq!(
                calls.len(),
                count,
                "captured absent/invalid role never gains latest configuration"
            );
        }
        assert_eq!(runtime.global_monty_owner.client().root_identity(), root);
    }
    shutdown_shared_runtime(runtime).await.unwrap();
    stop.cancel();
    server.await.unwrap();
}
