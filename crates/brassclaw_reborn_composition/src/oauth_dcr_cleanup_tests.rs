use super::tests::{DcrSetupEgress, TestObligationHandler, sample_auth_scope, test_config};
use super::*;
use brassclaw_auth::{AuthFlowStatus, DcrCleanupState, DcrRegistrationState};
use brassclaw_host_api::{RuntimeHttpEgressError, RuntimeHttpEgressResponse};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

struct PausedRegistrationEgress {
    inner: CleanupEgress,
    entered: tokio::sync::Semaphore,
    released: tokio::sync::Semaphore,
}
#[async_trait]
impl RuntimeHttpEgress for PausedRegistrationEgress {
    async fn execute(
        &self,
        request: RuntimeHttpEgressRequest,
    ) -> Result<RuntimeHttpEgressResponse, RuntimeHttpEgressError> {
        if request.method == NetworkMethod::Post {
            self.entered.add_permits(1);
            self.released.acquire().await.unwrap().forget();
        }
        self.inner.execute(request).await
    }
}

#[tokio::test]
async fn cancellation_while_dcr_post_is_running_captures_return_and_cleans_same_original() {
    let egress = Arc::new(PausedRegistrationEgress {
        inner: CleanupEgress::new(204, b"", false),
        entered: tokio::sync::Semaphore::new(0),
        released: tokio::sync::Semaphore::new(0),
    });
    let provider = OAuthDcrProvider::new(
        test_config(),
        egress.clone(),
        Arc::new(brassclaw_secrets::InMemorySecretStore::new()),
        Arc::new(TestObligationHandler),
    )
    .unwrap();
    let manager: Arc<dyn AuthFlowManager> =
        Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
    let request = DcrOriginalStart {
        requirement_setup: None,
        scope: sample_auth_scope(),
        flow_id: AuthFlowId::new(),
        account_label: CredentialAccountLabel::new("cancel-during-post").unwrap(),
        provider_scopes: vec![],
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        expires_at: Utc::now() + ChronoDuration::seconds(DCR_FLOW_TTL_SECONDS),
    };
    let worker = {
        let provider = provider.clone();
        let manager = manager.clone();
        let request = request.clone();
        tokio::spawn(async move {
            provider
                .start_original_registration(&manager, request)
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(10), egress.entered.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    let before = manager
        .read_dcr_registration(&request.scope, request.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        before.action.outcome,
        brassclaw_auth::DcrRegistrationOutcome::Submitted { .. }
    ));
    manager
        .cancel_flow(&request.scope, request.flow_id)
        .await
        .unwrap();
    egress.released.add_permits(1);
    let flow = worker.await.unwrap().unwrap();
    assert_eq!(flow.status, AuthFlowStatus::Canceled);
    assert!(flow.challenge.is_none());
    let after = manager
        .read_dcr_registration(&request.scope, request.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.action.original, after.action.original);
    assert!(after.action.cleanup.remote_confirmed());
    assert_eq!(egress.inner.posts.load(Ordering::SeqCst), 1);
    assert_eq!(egress.inner.deletes.load(Ordering::SeqCst), 1);
    provider
        .start_original_registration(&manager, request)
        .await
        .unwrap();
    assert_eq!(egress.inner.posts.load(Ordering::SeqCst), 1);
    assert_eq!(egress.inner.deletes.load(Ordering::SeqCst), 1);
}

struct CleanupEgress {
    status: u16,
    body: Vec<u8>,
    unknown: bool,
    management_uri: String,
    malformed_client: bool,
    deletes: AtomicUsize,
    posts: AtomicUsize,
    sent_urls: Mutex<Vec<String>>,
}
impl CleanupEgress {
    fn new(status: u16, body: &[u8], unknown: bool) -> Self {
        Self {
            status,
            body: body.to_vec(),
            unknown,
            management_uri: "https://oauth.notion.com/register/dcr-client".into(),
            malformed_client: false,
            deletes: AtomicUsize::new(0),
            posts: AtomicUsize::new(0),
            sent_urls: Mutex::new(vec![]),
        }
    }
}
#[async_trait]
impl RuntimeHttpEgress for CleanupEgress {
    async fn execute(
        &self,
        request: RuntimeHttpEgressRequest,
    ) -> Result<RuntimeHttpEgressResponse, RuntimeHttpEgressError> {
        self.sent_urls.lock().unwrap().push(request.url.clone());
        if request.method == NetworkMethod::Delete {
            self.deletes.fetch_add(1, Ordering::SeqCst);
            assert_eq!(request.url, self.management_uri);
            assert_eq!(
                request
                    .headers
                    .iter()
                    .find(|(name, _)| name == "authorization")
                    .unwrap()
                    .1,
                "Bearer registration-token"
            );
            if self.unknown {
                return Err(RuntimeHttpEgressError::Network {
                    reason: "controlled lost DELETE response".into(),
                    request_bytes: 5,
                    response_bytes: 0,
                });
            }
            return Ok(RuntimeHttpEgressResponse {
                status: self.status,
                headers: vec![("set-cookie".into(), "private-return-header".into())],
                body: self.body.clone(),
                saved_body: None,
                request_bytes: 5,
                response_bytes: self.body.len() as u64,
                redaction_applied: false,
            });
        }
        if request.method == NetworkMethod::Post {
            self.posts.fetch_add(1, Ordering::SeqCst);
            let mut response = DcrSetupEgress.execute(request).await?;
            let client_id = if self.malformed_client {
                serde_json::json!([])
            } else {
                serde_json::json!("dcr-client")
            };
            response.body=serde_json::to_vec(&serde_json::json!({"client_id":client_id, "registration_access_token":"registration-token", "registration_client_uri":self.management_uri})).unwrap();
            response.response_bytes = response.body.len() as u64;
            return Ok(response);
        }
        DcrSetupEgress.execute(request).await
    }
}

#[tokio::test]
async fn malformed_client_reply_still_retains_and_cleans_valid_management_original() {
    let mut egress = CleanupEgress::new(204, b"", false);
    egress.malformed_client = true;
    let egress = Arc::new(egress);
    let provider = OAuthDcrProvider::new(
        test_config(),
        egress.clone(),
        Arc::new(brassclaw_secrets::InMemorySecretStore::new()),
        Arc::new(TestObligationHandler),
    )
    .unwrap();
    let manager: Arc<dyn AuthFlowManager> =
        Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
    let request = DcrOriginalStart {
        requirement_setup: None,
        scope: sample_auth_scope(),
        flow_id: AuthFlowId::new(),
        account_label: CredentialAccountLabel::new("malformed-client-original").unwrap(),
        provider_scopes: vec![],
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        expires_at: Utc::now() + ChronoDuration::seconds(DCR_FLOW_TTL_SECONDS),
    };
    assert!(
        provider
            .start_original_registration(&manager, request.clone())
            .await
            .is_err()
    );
    let before = manager
        .read_dcr_registration(&request.scope, request.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert!(before.flow.challenge.is_none());
    manager
        .cancel_flow(&request.scope, request.flow_id)
        .await
        .unwrap();
    let canceled = manager
        .read_dcr_registration(&request.scope, request.flow_id)
        .await
        .unwrap()
        .unwrap();
    provider
        .cleanup_original_registration(&manager, canceled)
        .await
        .unwrap();
    let after = manager
        .read_dcr_registration(&request.scope, request.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.action.original, after.action.original);
    assert!(after.action.cleanup.remote_confirmed());
    assert_eq!(egress.posts.load(Ordering::SeqCst), 1);
    assert_eq!(egress.deletes.load(Ordering::SeqCst), 1);
}

async fn original(
    provider: &OAuthDcrProvider,
    manager: &Arc<dyn AuthFlowManager>,
) -> DcrRegistrationState {
    let request = DcrOriginalStart {
        requirement_setup: None,
        scope: {
            let mut scope = sample_auth_scope();
            scope.surface = brassclaw_auth::AuthSurface::Web;
            scope
        },
        flow_id: AuthFlowId::new(),
        account_label: CredentialAccountLabel::new("cleanup-original").unwrap(),
        provider_scopes: vec![],
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        expires_at: Utc::now() + ChronoDuration::seconds(DCR_FLOW_TTL_SECONDS),
    };
    let flow = provider
        .start_original_registration(manager, request)
        .await
        .unwrap();
    assert_eq!(flow.status, AuthFlowStatus::AwaitingUser);
    manager.cancel_flow(&flow.scope, flow.id).await.unwrap();
    manager
        .read_dcr_registration(&flow.scope, flow.id)
        .await
        .unwrap()
        .unwrap()
}

async fn verify_cleanup(
    first: Arc<dyn AuthFlowManager>,
    second: Arc<dyn AuthFlowManager>,
    store: Arc<dyn SecretStore>,
) {
    for (status, body, unknown) in [
        (204, b"".as_slice(), false),
        (204, b"{}".as_slice(), false),
        (200, b"{}".as_slice(), false),
        (405, b"not supported".as_slice(), false),
        (401, b"denied".as_slice(), false),
        (204, b"".as_slice(), true),
    ] {
        let egress = Arc::new(CleanupEgress::new(status, body, unknown));
        let provider = OAuthDcrProvider::new(
            test_config(),
            egress.clone(),
            store.clone(),
            Arc::new(TestObligationHandler),
        )
        .unwrap();
        let before = original(&provider, &first).await;
        let (a, b) = tokio::join!(
            provider.cleanup_original_registration(&first, before.clone()),
            provider.cleanup_original_registration(&second, before.clone())
        );
        if unknown {
            assert!(a.is_err() || b.is_err());
        } else {
            a.unwrap();
            b.unwrap();
        }
        let after = second
            .read_dcr_registration(&before.flow.scope, before.flow.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(before.action.original, after.action.original);
        assert_eq!(after.flow.status, AuthFlowStatus::Canceled);
        assert_eq!(
            after.action.cleanup.remote_confirmed(),
            status == 204 && body.is_empty() && !unknown
        );
        if unknown {
            assert!(matches!(
                after.action.cleanup,
                DcrCleanupState::Unknown { .. }
            ));
        } else {
            let DcrCleanupState::Returned {
                status: actual,
                empty_body,
                response,
                ..
            } = &after.action.cleanup
            else {
                panic!("actual cleanup return was not persisted");
            };
            assert_eq!(*actual, status);
            assert_eq!(*empty_body, body.is_empty());
            if after.action.cleanup.remote_confirmed() {
                assert!(after.action.retirement.as_ref().unwrap().confirmed());
                let metadata = store
                    .custody(&after.flow.scope.resource, &response.handle)
                    .await
                    .unwrap()
                    .unwrap();
                assert!(
                    matches!(&metadata.state,brassclaw_secrets::SecretCustodyState::RetiredOriginal {original_reference,..} if original_reference==&response.reference)
                );
                for entry in &after.action.retirement.as_ref().unwrap().entries {
                    let metadata = store
                        .custody(&after.flow.scope.resource, &entry.selection.handle)
                        .await
                        .unwrap()
                        .unwrap();
                    let brassclaw_auth::DcrRetirementOutcome::Confirmed { closed_reference } =
                        &entry.outcome
                    else {
                        panic!("missing local receipt");
                    };
                    assert_eq!(closed_reference, &metadata.reference);
                    assert!(
                        store
                            .lease_once(&after.flow.scope.resource, &entry.selection.handle)
                            .await
                            .is_err()
                    );
                }
            } else {
                let captured = provider
                    .read_dcr_material(
                        &after.action.original,
                        response.clone(),
                        "cleanup-response-read",
                    )
                    .await
                    .unwrap();
                let captured =
                    super::private_material::DcrResponseRecord::decode(&captured).unwrap();
                assert_eq!(captured.status, status);
                assert_eq!(captured.body().unwrap().expose_secret(), body);
                assert_eq!(
                    captured.headers[0].1.expose_secret(),
                    "private-return-header"
                );
            }
        }
        provider
            .cleanup_original_registration(&second, after)
            .await
            .unwrap();
        assert_eq!(egress.posts.load(Ordering::SeqCst), 1);
        assert_eq!(
            egress.deletes.load(Ordering::SeqCst),
            1,
            "no replay after known or unknown cleanup return"
        );
    }
}

#[tokio::test]
async fn canceled_dcr_cleanup_records_complete_returns_without_replaying_effects() {
    let auth = Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
    verify_cleanup(
        auth.clone(),
        auth,
        Arc::new(brassclaw_secrets::InMemorySecretStore::new()),
    )
    .await;
}

#[tokio::test]
async fn local_dcr_retirement_persists_unresolved_and_reconciles_lost_secret_ack_without_remote_replay()
 {
    use super::tests::FaultingDcrSecretStore;
    for after_commit in [false, true] {
        let store = Arc::new(FaultingDcrSecretStore::new_at(usize::MAX, false));
        let egress = Arc::new(CleanupEgress::new(204, b"", false));
        let provider = OAuthDcrProvider::new(
            test_config(),
            egress.clone(),
            store.clone(),
            Arc::new(TestObligationHandler),
        )
        .unwrap();
        let manager: Arc<dyn AuthFlowManager> =
            Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
        let before = original(&provider, &manager).await;
        store.fail_next_closure(after_commit);
        provider
            .cleanup_original_registration(&manager, before.clone())
            .await
            .unwrap();
        let pending = manager
            .read_dcr_registration(&before.flow.scope, before.flow.id)
            .await
            .unwrap()
            .unwrap();
        assert!(pending.action.cleanup.remote_confirmed());
        assert_eq!(
            pending.action.retirement.as_ref().unwrap().confirmed(),
            after_commit,
            "a committed lost secret ack is verified; a rejected write remains unresolved"
        );
        if !after_commit {
            assert!(matches!(
                pending.cleanup_projection().local,
                brassclaw_auth::DcrLocalCleanupStatus::Unresolved
            ));
            assert!(pending.cleanup_projection().can_reconcile);
        }
        provider
            .cleanup_original_registration(&manager, pending)
            .await
            .unwrap();
        let after = manager
            .read_dcr_registration(&before.flow.scope, before.flow.id)
            .await
            .unwrap()
            .unwrap();
        assert!(after.action.retirement.as_ref().unwrap().confirmed());
        assert!(!after.cleanup_projection().can_reconcile);
        assert_eq!(after.action.original, before.action.original);
        assert_eq!(egress.posts.load(Ordering::SeqCst), 1);
        assert_eq!(egress.deletes.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn canceled_pre_dispatch_original_closes_all_local_names_without_issuer_effects() {
    use super::tests::FaultingDcrSecretStore;
    let store = Arc::new(FaultingDcrSecretStore::new_at(2, false));
    let egress = Arc::new(CleanupEgress::new(204, b"", false));
    let provider = OAuthDcrProvider::new(
        test_config(),
        egress.clone(),
        store.clone(),
        Arc::new(TestObligationHandler),
    )
    .unwrap();
    let manager: Arc<dyn AuthFlowManager> =
        Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
    let request = DcrOriginalStart {
        requirement_setup: None,
        scope: sample_auth_scope(),
        flow_id: AuthFlowId::new(),
        account_label: CredentialAccountLabel::new("canceled-before-post").unwrap(),
        provider_scopes: vec![],
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        expires_at: Utc::now() + ChronoDuration::seconds(DCR_FLOW_TTL_SECONDS),
    };
    assert!(
        provider
            .start_original_registration(&manager, request.clone())
            .await
            .is_err()
    );
    manager
        .cancel_flow(&request.scope, request.flow_id)
        .await
        .unwrap();
    let canceled = manager
        .read_dcr_registration(&request.scope, request.flow_id)
        .await
        .unwrap()
        .unwrap();
    provider
        .cleanup_original_registration(&manager, canceled)
        .await
        .unwrap();
    let after = manager
        .read_dcr_registration(&request.scope, request.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        after.action.cleanup,
        brassclaw_auth::DcrCleanupState::NotRegistered { .. }
    ));
    assert!(after.action.retirement.as_ref().unwrap().confirmed());
    for slot in brassclaw_auth::DcrArtifactSlot::ALL {
        let handle = after.action.original.artifact_handle(slot).unwrap();
        assert!(
            store
                .put_original(
                    request.scope.resource.clone(),
                    handle,
                    "late canceled producer material".into(),
                    serde_json::json!({})
                )
                .await
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(egress.posts.load(Ordering::SeqCst), 0);
    assert_eq!(egress.deletes.load(Ordering::SeqCst), 0);
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_partial_dcr_retirement_survives_lost_plan_ack_and_restarted_owners_without_replaying_remote_effects()
 {
    use brassclaw_auth::{DcrArtifactSlot, DcrRetirementOutcome};
    use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
    use brassclaw_secrets::PgSecretStore;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let sql = rig.pool.get().await.unwrap();
    sql.batch_execute(crate::pg_auth_product_services::MIGRATIONS)
        .await
        .unwrap();
    let key = "controlled-native-interrupted-dcr-retirement-key";
    let namespace = "native-interrupted-dcr-retirement";
    let store =
        Arc::new(PgSecretStore::new(rig.pool.as_ref().clone(), key.into(), namespace).unwrap());
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![br#""retirement""#.to_vec(), br#""pending""#.to_vec()],
            vec![],
            AckBoundary::IdleReady,
        )],
    )
    .await
    .unwrap();
    let first: Arc<dyn AuthFlowManager> =
        Arc::new(crate::pg_auth_product_services::PgAuthProductServices::new(
            transport.pool.clone(),
            store.clone(),
        ));
    let egress = Arc::new(CleanupEgress::new(204, b"", false));
    let provider = OAuthDcrProvider::new(
        test_config(),
        egress.clone(),
        store.clone(),
        Arc::new(TestObligationHandler),
    )
    .unwrap();
    let before = original(&provider, &first).await;
    // Reject only the selected preparation's actual close, leaving other slots
    // able to commit receipts. No fake secret-store result or account is used.
    sql.batch_execute("CREATE FUNCTION reject_dcr_preparation_close() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF OLD.name LIKE '%-oauth-dcr-flow-preparation-original-%' THEN RAISE EXCEPTION 'controlled preparation close rejection'; END IF; RETURN NEW; END; $$; CREATE TRIGGER reject_dcr_preparation_close BEFORE UPDATE ON brassclaw_secrets FOR EACH ROW EXECUTE FUNCTION reject_dcr_preparation_close();").await.unwrap();
    transport.arm(1).unwrap();
    provider
        .cleanup_original_registration(&first, before.clone())
        .await
        .unwrap();
    assert_eq!(
        transport.cut_count(),
        1,
        "the original retirement plan must commit despite its lost reply"
    );
    let pending = first
        .read_dcr_registration(&before.flow.scope, before.flow.id)
        .await
        .unwrap()
        .unwrap();
    assert!(pending.action.cleanup.remote_confirmed());
    let plan = pending.action.retirement.as_ref().unwrap();
    assert!(!plan.confirmed());
    assert!(plan.entries.iter().any(|entry| entry.selection.slot
        == DcrArtifactSlot::PreparationOriginal
        && matches!(entry.outcome, DcrRetirementOutcome::Unresolved { .. })));
    let confirmed = plan
        .entries
        .iter()
        .filter(|entry| matches!(entry.outcome, DcrRetirementOutcome::Confirmed { .. }))
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        !confirmed.is_empty(),
        "actual persisted partial progress is required"
    );
    assert_eq!(
        pending.cleanup_projection().local,
        brassclaw_auth::DcrLocalCleanupStatus::Unresolved
    );
    assert!(pending.cleanup_projection().can_reconcile);
    drop(first);
    drop(provider);
    drop(store);
    sql.batch_execute("DROP TRIGGER reject_dcr_preparation_close ON brassclaw_secrets; DROP FUNCTION reject_dcr_preparation_close();").await.unwrap();
    let store =
        Arc::new(PgSecretStore::new(rig.pool.as_ref().clone(), key.into(), namespace).unwrap());
    let restarted: Arc<dyn AuthFlowManager> =
        Arc::new(crate::pg_auth_product_services::PgAuthProductServices::new(
            rig.pool.clone(),
            store.clone(),
        ));
    let provider = OAuthDcrProvider::new(
        test_config(),
        egress.clone(),
        store.clone(),
        Arc::new(TestObligationHandler),
    )
    .unwrap();
    let restored = restarted
        .read_dcr_registration(&before.flow.scope, before.flow.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored.action, pending.action);
    provider
        .cleanup_original_registration(&restarted, restored)
        .await
        .unwrap();
    let after = restarted
        .read_dcr_registration(&before.flow.scope, before.flow.id)
        .await
        .unwrap()
        .unwrap();
    assert!(after.action.retirement.as_ref().unwrap().confirmed());
    assert_eq!(after.action.original, before.action.original);
    for earlier in confirmed {
        assert_eq!(
            after
                .action
                .retirement
                .as_ref()
                .unwrap()
                .entries
                .iter()
                .find(|entry| entry.selection.slot == earlier.selection.slot),
            Some(&earlier),
            "a completed generation closure must never be replayed or rewritten"
        );
    }
    for entry in &after.action.retirement.as_ref().unwrap().entries {
        let metadata = store
            .custody(&before.flow.scope.resource, &entry.selection.handle)
            .await
            .unwrap()
            .unwrap();
        assert!(
            crate::auth_dcr_retirement::closure(&after.action, &entry.selection)
                .accepts(&before.flow.scope.resource, &metadata)
        );
    }
    provider
        .cleanup_original_registration(&restarted, after.clone())
        .await
        .unwrap();
    assert_eq!(
        restarted
            .read_dcr_registration(&before.flow.scope, before.flow.id)
            .await
            .unwrap()
            .unwrap()
            .action,
        after.action,
        "completed reconciliation must not churn its receipt"
    );
    assert_eq!(egress.posts.load(Ordering::SeqCst), 1);
    assert_eq!(egress.deletes.load(Ordering::SeqCst), 1);
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_dcr_retirement_refines_a_late_alias_and_accepts_only_the_exact_original_plan_retry()
{
    use brassclaw_auth::{
        DcrArtifactExpectation, DcrArtifactSlot, DcrCleanupState, DcrRetirementUpdate,
    };
    use brassclaw_secrets::PgSecretStore;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let sql = rig.pool.get().await.unwrap();
    sql.batch_execute(crate::pg_auth_product_services::MIGRATIONS)
        .await
        .unwrap();
    let store = Arc::new(
        PgSecretStore::new(
            rig.pool.as_ref().clone(),
            "controlled-native-dcr-late-alias-key".into(),
            "native-dcr-late-alias",
        )
        .unwrap(),
    );
    let manager: Arc<dyn AuthFlowManager> =
        Arc::new(crate::pg_auth_product_services::PgAuthProductServices::new(
            rig.pool.clone(),
            store.clone(),
        ));
    let egress = Arc::new(CleanupEgress::new(204, b"", false));
    let provider = OAuthDcrProvider::new(
        test_config(),
        egress.clone(),
        store.clone(),
        Arc::new(TestObligationHandler),
    )
    .unwrap();
    let before = original(&provider, &manager).await;
    // Hold the first actual Plan write after its complete input validation but
    // before commit. The independent secret writer can now win the real race.
    // This is a test-only SQL barrier, not a production hook or fake store.
    sql.batch_execute("CREATE FUNCTION pause_dcr_retirement_plan() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF OLD.data#>'{auth_custody,dcr_registration,retirement}' IS NULL AND NEW.data#>'{auth_custody,dcr_registration,retirement}' IS NOT NULL THEN PERFORM pg_advisory_xact_lock(838860,731); END IF; RETURN NEW; END; $$; CREATE TRIGGER pause_dcr_retirement_plan BEFORE UPDATE ON brassclaw_product_auth_flows FOR EACH ROW EXECUTE FUNCTION pause_dcr_retirement_plan();").await.unwrap();
    let mut gate_client = rig.pool.get().await.unwrap();
    let gate = gate_client.transaction().await.unwrap();
    gate.query_one("SELECT pg_advisory_xact_lock(838860,731)", &[])
        .await
        .unwrap();
    let worker = {
        let provider = provider.clone();
        let manager = manager.clone();
        let before = before.clone();
        tokio::spawn(async move {
            provider
                .cleanup_original_registration(&manager, before)
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(20),async {
        loop {
            let waiting:bool=sql.query_one("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND classid=838860::oid AND objid=731::oid AND objsubid=2 AND NOT granted)",&[]).await.unwrap().get(0);
            if waiting {break;}
            if worker.is_finished() {panic!("cleanup ended before its actual Plan write reached the barrier");}
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("the real native Plan write must reach the SQL barrier");
    let committed = manager
        .read_dcr_registration(&before.flow.scope, before.flow.id)
        .await
        .unwrap()
        .unwrap();
    assert!(committed.action.cleanup.remote_confirmed());
    assert!(
        committed.action.retirement.is_none(),
        "a blocked uncommitted Plan is not a persisted selection"
    );
    let mut original_plan = Vec::new();
    for slot in DcrArtifactSlot::ALL {
        original_plan.push(
            crate::auth_dcr_retirement::select(store.as_ref(), &committed.action, slot)
                .await
                .unwrap(),
        );
    }
    let absent = original_plan
        .iter()
        .find(|selected| selected.slot == DcrArtifactSlot::CleanupResponseRead)
        .unwrap()
        .clone();
    assert!(matches!(
        absent.expected,
        DcrArtifactExpectation::Absent { .. }
    ));
    let DcrCleanupState::Returned { response, .. } = &committed.action.cleanup else {
        panic!("actual remote return missing");
    };
    let late = provider
        .retain_dcr_output(
            &before.flow.scope.resource,
            absent.handle.clone(),
            response.clone(),
        )
        .await
        .unwrap();
    gate.commit().await.unwrap();
    tokio::time::timeout(Duration::from_secs(20), worker)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    sql.batch_execute("DROP TRIGGER pause_dcr_retirement_plan ON brassclaw_product_auth_flows; DROP FUNCTION pause_dcr_retirement_plan();").await.unwrap();
    let after = manager
        .read_dcr_registration(&before.flow.scope, before.flow.id)
        .await
        .unwrap()
        .unwrap();
    let plan = after.action.retirement.as_ref().unwrap();
    assert!(plan.confirmed());
    let refined = plan
        .entries
        .iter()
        .find(|entry| entry.selection.slot == DcrArtifactSlot::CleanupResponseRead)
        .unwrap();
    assert_eq!(refined.prior_absence.as_ref(), Some(&absent));
    assert!(
        matches!(&refined.selection.expected,DcrArtifactExpectation::Retained {reference,..} if reference==&late.reference)
    );
    let repeated = manager
        .update_dcr_retirement(
            &before.action.original,
            DcrRetirementUpdate::Plan(original_plan.clone()),
        )
        .await
        .unwrap();
    assert_eq!(
        repeated.action, after.action,
        "exact original Plan retry must remain read-only after refinement/closure"
    );
    let duplicate = vec![original_plan[0].clone(); DcrArtifactSlot::ALL.len()];
    assert!(
        manager
            .update_dcr_retirement(
                &before.action.original,
                DcrRetirementUpdate::Plan(duplicate)
            )
            .await
            .is_err(),
        "readback cannot bypass complete unique slot coverage"
    );
    let mut changed = original_plan;
    changed
        .iter_mut()
        .find(|selected| selected.slot == DcrArtifactSlot::CleanupResponseRead)
        .unwrap()
        .handle = SecretHandle::new("foreign-retirement-alias").unwrap();
    assert!(
        manager
            .update_dcr_retirement(&before.action.original, DcrRetirementUpdate::Plan(changed))
            .await
            .is_err()
    );
    assert_eq!(
        manager
            .read_dcr_registration(&before.flow.scope, before.flow.id)
            .await
            .unwrap()
            .unwrap()
            .action,
        after.action
    );
    assert_eq!(egress.posts.load(Ordering::SeqCst), 1);
    assert_eq!(egress.deletes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn canceled_dcr_never_leaks_management_bearer_to_untrusted_reply_endpoint() {
    for uri in [
        "https://attacker.invalid/register/client",
        "http://oauth.notion.com/register/client",
        "https://user:pass@oauth.notion.com/register/client",
        "https://oauth.notion.com/register/client#fragment",
    ] {
        let mut egress = CleanupEgress::new(204, b"", false);
        egress.management_uri = uri.into();
        let egress = Arc::new(egress);
        let provider = OAuthDcrProvider::new(
            test_config(),
            egress.clone(),
            Arc::new(brassclaw_secrets::InMemorySecretStore::new()),
            Arc::new(TestObligationHandler),
        )
        .unwrap();
        let manager: Arc<dyn AuthFlowManager> =
            Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
        let before = original(&provider, &manager).await;
        provider
            .cleanup_original_registration(&manager, before.clone())
            .await
            .unwrap();
        let after = manager
            .read_dcr_registration(&before.flow.scope, before.flow.id)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            after.action.cleanup,
            DcrCleanupState::Unavailable {
                issue: brassclaw_auth::DcrCleanupUnavailable::InvalidManagementEndpoint,
                ..
            }
        ));
        assert_eq!(egress.deletes.load(Ordering::SeqCst), 0);
        assert!(
            !egress
                .sent_urls
                .lock()
                .unwrap()
                .iter()
                .any(|sent| sent == uri)
        );
    }
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_dcr_cleanup_claims_and_returns_survive_independent_pg_and_cas_owners() {
    use brassclaw_filesystem::{PostgresRootFilesystem, ScopedFilesystem};
    use brassclaw_host_api::{MountAlias, MountGrant, MountPermissions, MountView, VirtualPath};
    use brassclaw_secrets::{FilesystemSecretStore, PgSecretStore, SecretsCrypto};
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    rig.pool
        .get()
        .await
        .unwrap()
        .batch_execute(crate::pg_auth_product_services::MIGRATIONS)
        .await
        .unwrap();
    let store = Arc::new(
        PgSecretStore::new(
            rig.pool.as_ref().clone(),
            "controlled-cleanup-native-master-key".into(),
            "dcr-cleanup-contract",
        )
        .unwrap(),
    );
    verify_cleanup(
        Arc::new(crate::pg_auth_product_services::PgAuthProductServices::new(
            rig.pool.clone(),
            store.clone(),
        )),
        Arc::new(crate::pg_auth_product_services::PgAuthProductServices::new(
            rig.pool.clone(),
            store.clone(),
        )),
        store,
    )
    .await;
    let root = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    root.run_migrations().await.unwrap();
    let view = MountView::new(vec![MountGrant::new(
        MountAlias::new("/secrets").unwrap(),
        VirtualPath::new("/secrets/dcr-cleanup-contract").unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    let fs = Arc::new(ScopedFilesystem::with_fixed_view(root, view));
    let store: Arc<dyn SecretStore> = Arc::new(FilesystemSecretStore::new(
        fs.clone(),
        Arc::new(SecretsCrypto::new("controlled-cleanup-filesystem-master-key".into()).unwrap()),
    ));
    verify_cleanup(
        Arc::new(
            crate::product_auth_durable::FilesystemAuthProductServices::new(
                fs.clone(),
                store.clone(),
            ),
        ),
        Arc::new(
            crate::product_auth_durable::FilesystemAuthProductServices::new(fs, store.clone()),
        ),
        store,
    )
    .await;
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_cleanup_claim_ack_loss_never_becomes_delete_authority_from_readback() {
    use brassclaw_auth::{DcrCleanupAdmission, DcrCleanupPrepared, DcrRegistrationOutcome};
    use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
    use brassclaw_secrets::PgSecretStore;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    rig.pool
        .get()
        .await
        .unwrap()
        .batch_execute(crate::pg_auth_product_services::MIGRATIONS)
        .await
        .unwrap();
    let store = Arc::new(
        PgSecretStore::new(
            rig.pool.as_ref().clone(),
            "controlled-cleanup-claim-ack-master-key".into(),
            "cleanup-claim-ack-loss",
        )
        .unwrap(),
    );
    let reader: Arc<dyn AuthFlowManager> =
        Arc::new(crate::pg_auth_product_services::PgAuthProductServices::new(
            rig.pool.clone(),
            store.clone(),
        ));
    let egress = Arc::new(CleanupEgress::new(204, b"", false));
    let provider = OAuthDcrProvider::new(
        test_config(),
        egress.clone(),
        store.clone(),
        Arc::new(TestObligationHandler),
    )
    .unwrap();
    let before = original(&provider, &reader).await;
    let DcrRegistrationOutcome::Returned { response, .. } = &before.action.outcome else {
        panic!("original registration reply missing");
    };
    let management = provider
        .retain_dcr_original(
            &before.flow.scope.resource,
            flow_secret_handle(&provider.spec, before.flow.id, "cleanup-management").unwrap(),
            super::private_material::DcrManagementRecord {
                uri: "https://oauth.notion.com/register/dcr-client".into(),
                token: "registration-token".into(),
            }
            .encode()
            .unwrap(),
            before.action.cleanup_management_context().unwrap(),
        )
        .await
        .unwrap();
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![
                before.flow.id.to_string().into_bytes(),
                br#""submitted""#.to_vec(),
            ],
            vec![],
            AckBoundary::IdleReady,
        )],
    )
    .await
    .unwrap();
    let writer: Arc<dyn AuthFlowManager> = Arc::new(
        crate::pg_auth_product_services::PgAuthProductServices::new(transport.pool.clone(), store),
    );
    transport.arm(1).unwrap();
    let admission = writer
        .claim_dcr_cleanup(DcrCleanupPrepared {
            original: before.action.original.clone(),
            registration_response: response.clone(),
            management,
        })
        .await
        .unwrap();
    assert!(
        matches!(admission, DcrCleanupAdmission::Original(_)),
        "only a fresh acknowledged claim permits DELETE"
    );
    assert_eq!(transport.cut_count(), 1);
    let actual = reader
        .read_dcr_registration(&before.flow.scope, before.flow.id)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        actual.action.cleanup,
        DcrCleanupState::Submitted { .. }
    ));
    provider
        .cleanup_original_registration(&reader, actual)
        .await
        .unwrap();
    assert_eq!(egress.posts.load(Ordering::SeqCst), 1);
    assert_eq!(egress.deletes.load(Ordering::SeqCst), 0);
}
