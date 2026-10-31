//! Native acknowledged settings + actual PG Auth original and host/network
//! denial. The cached issuer fixture is data, never a successful Tool response.
use super::tests::{sample_auth_scope, test_config};
use super::*;
use brassclaw_host_api::{RuntimeHttpEgressError, RuntimeHttpEgressResponse};
use brassclaw_host_runtime::{BuiltinObligationServices, HostPrimitiveSettingsSource};
use brassclaw_product_workflow::MontyMemoryMode;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::Notify;

#[derive(Debug, PartialEq, Eq)]
struct ObservedBounds {
    method: NetworkMethod,
    timeout: Option<u32>,
    response_bytes: Option<u64>,
    egress_bytes: Option<u64>,
}
struct ObservedEgress {
    inner: Arc<dyn RuntimeHttpEgress>,
    observations: Mutex<Vec<ObservedBounds>>,
    failures: Mutex<Vec<&'static str>>,
    pause_once: AtomicBool,
    entered: Notify,
    released: Notify,
}
impl fmt::Debug for ObservedEgress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObservedEgress").finish_non_exhaustive()
    }
}
#[async_trait]
impl RuntimeHttpEgress for ObservedEgress {
    async fn execute(
        &self,
        request: RuntimeHttpEgressRequest,
    ) -> Result<RuntimeHttpEgressResponse, RuntimeHttpEgressError> {
        self.observations.lock().unwrap().push(ObservedBounds {
            method: request.method,
            timeout: request.timeout_ms,
            response_bytes: request.response_body_limit,
            egress_bytes: request.network_policy.max_egress_bytes,
        });
        if self.pause_once.swap(false, Ordering::AcqRel) {
            self.entered.notify_one();
            self.released.notified().await;
        }
        let result = self.inner.execute(request).await;
        if let Err(error) = &result {
            self.failures
                .lock()
                .unwrap()
                .push(error.stable_runtime_reason());
        }
        result
    }
}
fn metadata_fixture() -> CachedAuthorizationServerMetadata {
    CachedAuthorizationServerMetadata {
        metadata: AuthorizationServerMetadata {
            authorization_endpoint: "https://127.0.0.1:9/authorize".into(),
            token_endpoint: "https://127.0.0.1:9/token".into(),
            registration_endpoint: "https://127.0.0.1:9/register".into(),
        },
        stored_at: Instant::now(),
        ttl_secs: 120,
    }
}

async fn reject_flow_insert<F>(
    pool: &brassclaw_pg::PgPool,
    flow_id: AuthFlowId,
    operation: F,
) -> Result<(), AuthProductError>
where
    F: std::future::Future<Output = Result<(), AuthProductError>>,
{
    let client = pool.get().await.unwrap();
    client
        .batch_execute(&format!(
            "CREATE FUNCTION reject_original_gate_flow() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'controlled original DCR flow publication failure'; END $$;
         CREATE TRIGGER reject_original_gate_flow BEFORE INSERT ON brassclaw_product_auth_flows
         FOR EACH ROW WHEN (NEW.id = '{flow_id}') EXECUTE FUNCTION reject_original_gate_flow();"
        ))
        .await
        .unwrap();
    let result = operation.await;
    // Remove the real fault before assertions or retrying. The native rig is
    // private to this test, including if a panic tears it down.
    client
        .batch_execute(
            "DROP TRIGGER reject_original_gate_flow ON brassclaw_product_auth_flows;
         DROP FUNCTION reject_original_gate_flow();",
        )
        .await
        .unwrap();
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_dcr_http_limits_refresh_between_requests_and_do_not_spend_unbound_claim() {
    use crate::{RebornRuntimeIdentity, RebornRuntimeInput, build_reborn_runtime};
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(
        RebornRuntimeInput::from_services(
            rig.build_input("dcr-http-owner", directory.path())
                .with_tenant_id("default")
                .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "default".into(),
            ..RebornRuntimeIdentity::reborn_cli()
        }),
    ))
    .await
    .unwrap();
    let settings = runtime.webui_monty_settings_store();
    let original = settings.get("default", "default").await.unwrap();
    let source = runtime
        .services()
        .primitive_settings
        .as_ref()
        .unwrap()
        .clone();
    let secrets = runtime.services().secret_store.clone();
    let services = BuiltinObligationServices::new(
        Arc::new(brassclaw_events::InMemoryAuditSink::new()),
        secrets.clone(),
        Arc::new(brassclaw_resources::InMemoryResourceGovernor::new()),
    );
    let egress = Arc::new(ObservedEgress {
        inner: Arc::new(services.host_http_egress(
            brassclaw_network::PolicyNetworkHttpEgress::new(
                brassclaw_network::ReqwestNetworkTransport::default(),
            ),
        )),
        observations: Mutex::new(vec![]),
        failures: Mutex::new(vec![]),
        pause_once: AtomicBool::new(true),
        entered: Notify::new(),
        released: Notify::new(),
    });
    let mut config = test_config();
    config.spec.resource = Some("https://127.0.0.1:9/mcp");
    let provider = OAuthDcrProvider::new(
        config,
        egress.clone(),
        secrets,
        Arc::new(services.obligation_handler()),
    )
    .unwrap()
    .with_primitive_settings_source(Some(source.clone()));
    let mut first = original.execution_limits;
    first.oauth_dcr_timeout_millis = 47000;
    first.oauth_dcr_response_body_bytes = 65539;
    let changed = settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(
                serde_json::json!({"expected_revision":original.revision,"execution_limits":first}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let active = {
        let provider = provider.clone();
        tokio::spawn(async move {
            provider
                .discover_authorization_server(&sample_auth_scope().resource)
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(5), egress.entered.notified())
        .await
        .unwrap();
    let mut second = first;
    second.oauth_dcr_timeout_millis = 51000;
    second.oauth_dcr_response_body_bytes = 131075;
    let newer = settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(
                serde_json::json!({"expected_revision":changed.revision,"execution_limits":second}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let captured = source.snapshot().await.unwrap();
    assert_eq!(captured.revision, newer.revision);
    assert_eq!(captured.limits, second);
    let effective = settings
        .runtime_status()
        .await
        .unwrap()
        .unwrap()
        .execution_limits
        .unwrap();
    assert_eq!(effective.effective_revision, newer.revision);
    assert_eq!(effective.limits.oauth_dcr_timeout_millis, 51000);
    assert_eq!(effective.limits.oauth_dcr_response_body_bytes, 131075);
    egress.released.notify_one();
    assert!(active.await.unwrap().is_err());
    // The first admitted GET keeps its old pair; the later fallback GET must
    // observe the newly acknowledged pair. No fields may mix revisions.
    assert_eq!(
        *egress.observations.lock().unwrap(),
        vec![
            ObservedBounds {
                method: NetworkMethod::Get,
                timeout: Some(47000),
                response_bytes: Some(65539),
                egress_bytes: Some(65539)
            },
            ObservedBounds {
                method: NetworkMethod::Get,
                timeout: Some(51000),
                response_bytes: Some(131075),
                egress_bytes: Some(131075)
            },
        ]
    );
    assert!(
        egress
            .failures
            .lock()
            .unwrap()
            .iter()
            .all(|failure| *failure == "policy_denied")
    );
    // A validated metadata data fixture isolates the original native claim
    // boundary; it does not invent a registration or management response.
    *provider.metadata_cache.write().await = Some(metadata_fixture());
    let unbound = provider
        .clone()
        .with_primitive_settings_source(Some(Arc::new(
            crate::host_primitive_settings::AcknowledgedPrimitiveSettings::new(),
        )));
    let manager = runtime
        .services()
        .product_auth
        .as_ref()
        .unwrap()
        .flow_manager();
    let request = DcrOriginalStart {
        requirement_setup: None,
        scope: sample_auth_scope(),
        flow_id: AuthFlowId::new(),
        account_label: CredentialAccountLabel::new("real-native-dcr-limits").unwrap(),
        provider_scopes: vec![],
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        expires_at: Utc::now() + ChronoDuration::seconds(300),
    };
    // The original is accepted under a real acknowledged cache policy. Stop
    // its real flow insert once, then verify unavailable HTTP settings cannot
    // consume the original registration claim during recovery.
    assert_eq!(
        reject_flow_insert(rig.pool.as_ref(), request.flow_id, async {
            provider
                .start_original_registration(&manager, request.clone())
                .await
                .map(|_| ())
        })
        .await
        .unwrap_err(),
        AuthProductError::BackendUnavailable
    );
    assert!(
        unbound
            .start_original_registration(&manager, request.clone())
            .await
            .is_err()
    );
    let before = manager
        .read_dcr_registration(&request.scope, request.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        before.action.outcome,
        brassclaw_auth::DcrRegistrationOutcome::Prepared
    ));
    assert_eq!(
        egress.observations.lock().unwrap().len(),
        2,
        "unavailable policy must not dispatch or consume the original claim"
    );
    assert!(
        provider
            .start_original_registration(&manager, request.clone())
            .await
            .is_err()
    );
    let after = manager
        .read_dcr_registration(&request.scope, request.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.action.original, after.action.original);
    assert!(matches!(
        after.action.outcome,
        brassclaw_auth::DcrRegistrationOutcome::Unknown { .. }
    ));
    assert_eq!(egress.observations.lock().unwrap().len(), 3);
    // Unknown original readback cannot replay, even after the live owner becomes
    // unavailable. It must consult the retained outcome, not current settings.
    assert!(matches!(
        unbound
            .start_original_registration(&manager, request.clone())
            .await,
        Err(AuthProductError::BackendUnavailable)
    ));
    let retained = manager
        .read_dcr_registration(&request.scope, request.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.action, after.action);
    assert_eq!(egress.observations.lock().unwrap().len(), 3);
    let scope = sample_auth_scope().resource;
    let http = provider.capture_http_limits().await.unwrap();
    assert!(
        provider
            .execute_request(
                scope.clone(),
                NetworkMethod::Delete,
                "https://127.0.0.1:9/register/client",
                vec![],
                http,
                None
            )
            .await
            .is_err()
    );
    assert_eq!(
        &egress.observations.lock().unwrap()[2..],
        &[
            ObservedBounds {
                method: NetworkMethod::Post,
                timeout: Some(51000),
                response_bytes: Some(131075),
                egress_bytes: Some(131075)
            },
            ObservedBounds {
                method: NetworkMethod::Delete,
                timeout: Some(51000),
                response_bytes: Some(131075),
                egress_bytes: Some(131075)
            },
        ]
    );
    assert_eq!(*egress.failures.lock().unwrap(), vec!["policy_denied"; 4]);
    for (timeout, bytes) in [(0, 131075), (51000, 0), (51000, u64::MAX)] {
        let mut bad = second;
        bad.oauth_dcr_timeout_millis = timeout;
        bad.oauth_dcr_response_body_bytes = bytes;
        assert!(settings.upsert("default","default",&serde_json::from_value(serde_json::json!({"expected_revision":newer.revision,"execution_limits":bad})).unwrap()).await.is_err());
        assert_eq!(source.snapshot().await.unwrap().limits, second);
    }
    // Real metadata reuse is separately controlled from HTTP bounds and the
    // accepted registration. Cached metadata is fixture data, never Tool success.
    *provider.metadata_cache.write().await = Some(metadata_fixture());
    let requests_before_cache = egress.observations.lock().unwrap().len();
    assert_eq!(
        provider
            .discover_authorization_server(&scope)
            .await
            .unwrap()
            .registration_endpoint,
        "https://127.0.0.1:9/register"
    );
    assert_eq!(
        egress.observations.lock().unwrap().len(),
        requests_before_cache
    );
    assert!(unbound.discover_authorization_server(&scope).await.is_err());
    assert_eq!(
        egress.observations.lock().unwrap().len(),
        requests_before_cache
    );
    let mut revision = newer.revision;
    for (ttl, extra_requests) in [(0, 2), (900, 4), (u32::MAX, 6)] {
        let mut limits = second;
        limits.oauth_dcr_metadata_cache_ttl_secs = ttl;
        let saved = settings
            .upsert(
                "default",
                "default",
                &serde_json::from_value(
                    serde_json::json!({"expected_revision":revision,"execution_limits":limits}),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        revision = saved.revision;
        let acknowledged = source.snapshot().await.unwrap();
        assert_eq!(acknowledged.revision, revision);
        assert_eq!(acknowledged.limits, limits);
        let effective = settings
            .runtime_status()
            .await
            .unwrap()
            .unwrap()
            .execution_limits
            .unwrap();
        assert_eq!(effective.effective_revision, revision);
        assert_eq!(effective.limits.oauth_dcr_metadata_cache_ttl_secs, ttl);
        if ttl == u32::MAX {
            let mut old = metadata_fixture();
            old.ttl_secs = 1;
            *provider.metadata_cache.write().await = Some(old);
            // Actual monotonic age, with no synthetic clock or Instant addition.
            tokio::time::sleep(Duration::from_millis(1100)).await;
        }
        assert!(
            provider
                .discover_authorization_server(&scope)
                .await
                .is_err()
        );
        assert!(provider.metadata_cache.read().await.is_none());
        assert_eq!(
            egress.observations.lock().unwrap().len(),
            requests_before_cache + extra_requests
        );
        assert_eq!(
            manager
                .read_dcr_registration(&request.scope, request.flow_id)
                .await
                .unwrap()
                .unwrap()
                .action,
            after.action,
            "cache edits cannot renew or replay an accepted registration"
        );
    }
    settings.upsert("default","default",&serde_json::from_value(serde_json::json!({"expected_revision":revision,"execution_limits":original.execution_limits})).unwrap()).await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_dcr_gate_recovers_preparation_after_flow_insert_failure_without_rebinding() {
    use crate::{RebornRuntimeIdentity, RebornRuntimeInput, build_reborn_runtime};
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(
        RebornRuntimeInput::from_services(
            rig.build_input("dcr-gate-recovery", directory.path())
                .with_tenant_id("default")
                .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "default".into(),
            ..RebornRuntimeIdentity::reborn_cli()
        }),
    ))
    .await
    .unwrap();
    let source = runtime
        .services()
        .primitive_settings
        .as_ref()
        .unwrap()
        .clone();
    let services = BuiltinObligationServices::new(
        Arc::new(brassclaw_events::InMemoryAuditSink::new()),
        runtime.services().secret_store.clone(),
        Arc::new(brassclaw_resources::InMemoryResourceGovernor::new()),
    );
    let egress = Arc::new(ObservedEgress {
        inner: Arc::new(services.host_http_egress(
            brassclaw_network::PolicyNetworkHttpEgress::new(
                brassclaw_network::ReqwestNetworkTransport::default(),
            ),
        )),
        observations: Mutex::new(vec![]),
        failures: Mutex::new(vec![]),
        pause_once: AtomicBool::new(false),
        entered: Notify::new(),
        released: Notify::new(),
    });
    let mut config = test_config();
    config.spec.resource = Some("https://127.0.0.1:9/mcp");
    let provider = OAuthDcrProvider::new(
        config,
        egress.clone(),
        runtime.services().secret_store.clone(),
        Arc::new(services.obligation_handler()),
    )
    .unwrap()
    .with_primitive_settings_source(Some(source));
    let auth = runtime.services().product_auth.as_ref().unwrap();
    let manager = auth.flow_manager();
    let flows = auth.flow_record_source().unwrap();
    let resource = sample_auth_scope().resource;
    let owner = resource.user_id;
    let scope = TurnScope::new(
        resource.tenant_id,
        resource.agent_id,
        resource.project_id,
        resource.thread_id.unwrap(),
    );

    for foreign_setup in [false, true] {
        let run = TurnRunId::new();
        let gate = AuthGateRef::new("native-preparation-before-flow").unwrap();
        // Build the expected stable gate identity to inspect its real encrypted
        // custody even while the actual PG flow publication is deliberately absent.
        let seed = serde_json::to_vec(&(
            "brassclaw-dcr-gate-original/1",
            scope.tenant_id.as_str(),
            owner.as_str(),
            &scope.agent_id,
            &scope.project_id,
            scope.thread_id.as_str(),
            run.to_string(),
            gate.as_str(),
            provider.spec.provider_id,
        ))
        .unwrap();
        let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, &seed);
        let flow_id = AuthFlowId::from_uuid(id);
        let mut auth_scope = auth_scope_for_blocked_turn(&scope, &owner);
        auth_scope.resource.invocation_id =
            InvocationId::from_uuid(uuid::Uuid::new_v5(&id, b"dcr-auth-invocation/1"));
        let before_requests = egress.observations.lock().unwrap().len();
        // This is issuer metadata data only; no successful HTTP/Tool reply is
        // synthesized. The actual POST below reaches the native host/network guard.
        *provider.metadata_cache.write().await = Some(metadata_fixture());
        let first = reject_flow_insert(rig.pool.as_ref(), flow_id, async {
            if foreign_setup {
                provider
                    .start_original_registration(
                        &manager,
                        DcrOriginalStart {
                            requirement_setup: None,
                            scope: auth_scope.clone(),
                            flow_id,
                            account_label: provider.account_label.clone(),
                            provider_scopes: provider.scopes.clone(),
                            continuation: AuthContinuationRef::SetupOnly,
                            update_binding: None,
                            expires_at: Utc::now() + ChronoDuration::seconds(120),
                        },
                    )
                    .await
                    .map(|_| ())
            } else {
                provider
                    .challenge_for_blocked_gate(&manager, &flows, &scope, &owner, run, &gate)
                    .await
                    .map(|_| ())
            }
        })
        .await;
        assert_eq!(first.unwrap_err(), AuthProductError::BackendUnavailable);
        assert!(
            manager
                .get_flow(&auth_scope, flow_id)
                .await
                .unwrap()
                .is_none()
        );
        let original = provider
            .retained_preparation_original(&auth_scope, flow_id)
            .await
            .unwrap()
            .expect("actual encrypted preparation survived failed flow publication");
        assert_eq!(egress.observations.lock().unwrap().len(), before_requests);

        if foreign_setup {
            assert_eq!(
                provider
                    .challenge_for_blocked_gate(&manager, &flows, &scope, &owner, run, &gate)
                    .await
                    .unwrap_err(),
                AuthProductError::BackendConflict
            );
            assert!(
                manager
                    .get_flow(&auth_scope, flow_id)
                    .await
                    .unwrap()
                    .is_none()
            );
            assert_eq!(egress.observations.lock().unwrap().len(), before_requests);
        } else {
            let mut changed = provider.clone();
            changed.account_label = CredentialAccountLabel::new("later provider default").unwrap();
            changed.scopes = vec![ProviderScope::new("later-default-scope").unwrap()];
            let unbound = changed
                .clone()
                .with_primitive_settings_source(Some(Arc::new(
                    crate::host_primitive_settings::AcknowledgedPrimitiveSettings::new(),
                )));
            assert_eq!(
                unbound
                    .challenge_for_blocked_gate(&manager, &flows, &scope, &owner, run, &gate)
                    .await
                    .unwrap_err(),
                AuthProductError::BackendUnavailable
            );
            let prepared = manager
                .read_dcr_registration(&auth_scope, flow_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(prepared.action.original, original);
            assert_eq!(prepared.flow.expires_at, original.expires_at);
            assert!(matches!(
                prepared.action.outcome,
                brassclaw_auth::DcrRegistrationOutcome::Prepared
            ));
            assert_eq!(egress.observations.lock().unwrap().len(), before_requests);
            assert!(
                changed
                    .challenge_for_blocked_gate(&manager, &flows, &scope, &owner, run, &gate)
                    .await
                    .is_err()
            );
            let attempted = manager
                .read_dcr_registration(&auth_scope, flow_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(attempted.action.original, original);
            assert!(matches!(
                attempted.action.outcome,
                brassclaw_auth::DcrRegistrationOutcome::Unknown { .. }
            ));
            assert_eq!(
                egress.observations.lock().unwrap().len(),
                before_requests + 1
            );
            assert_eq!(
                egress.observations.lock().unwrap().last().unwrap().method,
                NetworkMethod::Post
            );
            assert_eq!(
                egress.failures.lock().unwrap().last(),
                Some(&"policy_denied")
            );
            assert!(
                unbound
                    .challenge_for_blocked_gate(&manager, &flows, &scope, &owner, run, &gate)
                    .await
                    .is_err()
            );
            assert_eq!(
                manager
                    .read_dcr_registration(&auth_scope, flow_id)
                    .await
                    .unwrap()
                    .unwrap()
                    .action,
                attempted.action
            );
            assert_eq!(
                egress.observations.lock().unwrap().len(),
                before_requests + 1
            );
        }
        assert_eq!(
            provider
                .retained_preparation_original(&auth_scope, flow_id)
                .await
                .unwrap(),
            Some(original)
        );
    }
    runtime.shutdown().await.unwrap();
}
