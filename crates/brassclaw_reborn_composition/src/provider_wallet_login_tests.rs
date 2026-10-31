//! Actual PG, one-shot external HTTP and ordinary live provider reload. The peer
//! controls authorization replies; this is not real wallet/signature qualification.
use super::super::tests::{Fixture, caller, file_unchanged, fixture, reference};
use super::*;
use brassclaw_llm::{ChatMessage, CompletionRequest, LlmProvider};
use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
use brassclaw_secrets::SecretMaterial;
use std::sync::atomic::Ordering;

const SIGNATURE: &str =
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==";
fn response(challenge: &NearAiWalletLoginStart) -> NearAiWalletLoginRequest {
    NearAiWalletLoginRequest {
        mutation_id: challenge.mutation_id,
        account_id: "alice.near".into(),
        public_key: "ed25519:11111111111111111111111111111111".into(),
        signature: SIGNATURE.into(),
        message: challenge.message.clone(),
        recipient: challenge.recipient.clone(),
        nonce: challenge.nonce.clone(),
        callback_url: challenge.callback_url.clone(),
    }
}
async fn start(f: &Fixture) -> NearAiWalletLoginStart {
    f.service
        .start_nearai_wallet_login(caller(&f.service.db_tenant_id), Default::default())
        .await
        .unwrap()
}
async fn finish(
    f: &Fixture,
    request: NearAiWalletLoginRequest,
) -> Result<NearAiWalletLoginResult, LlmConfigServiceError> {
    f.service
        .complete_nearai_wallet_login(caller(&f.service.db_tenant_id), request)
        .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_wallet_challenge_replaces_stored_key_once_and_recovers_original_commit() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![b"upsert".to_vec(), WALLET_LOGIN.as_bytes().to_vec()],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    let f = fixture(transport.pool.clone(), "native-wallet-original").await;
    f.service
        .keys
        .put("nearai", SecretMaterial::from("previous-provider-key"))
        .await
        .unwrap();
    let challenge = start(&f).await;
    assert_eq!(challenge.nonce.len(), 32);
    assert_eq!(&challenge.nonce[8..24], challenge.mutation_id.as_bytes());
    let signed = response(&challenge);
    assert!(!format!("{signed:?}").contains(SIGNATURE));
    transport.arm(1).unwrap();
    let result = finish(&f, signed.clone()).await.unwrap();
    assert!(result.active);
    assert_eq!(result.mutation_receipt.mutation_id, challenge.mutation_id);
    assert_eq!(transport.cut_count(), 1);
    assert_eq!(f.wallet_peer.requests.lock().await.len(), 1);
    let wire = f.wallet_peer.requests.lock().await[0].clone();
    assert_eq!(wire["payload"]["nonce"], json!(challenge.nonce));
    assert_eq!(wire["signed_message"]["signature"], SIGNATURE);
    let current = reference(&f).await;
    f.primary
        .complete(CompletionRequest::new(vec![ChatMessage::user(
            "wallet credential is canonical",
        )]))
        .await
        .unwrap();
    assert_eq!(&*f.model_auth.lock().await, &["Bearer wallet-issued-token"]);
    for _ in 0..2 {
        f.service.nearai_login_states.states.lock().await.clear();
        assert_eq!(
            finish(&f, signed.clone()).await.unwrap().mutation_receipt,
            result.mutation_receipt
        );
        assert_eq!(reference(&f).await, current);
    }
    assert_eq!(
        f.wallet_peer.requests.lock().await.len(),
        1,
        "completed original is never exchanged again"
    );
    let rebuilt = RebornLlmConfigService::new(
        f.service.keys.clone(),
        f.service.pg_pool.clone(),
        f.service.pg_provider_repo.clone(),
        &f.service.db_tenant_id,
    )
    .with_nearai_session(f.service.nearai_session.clone().unwrap())
    .with_reload_trigger(f.service.reload.clone().unwrap());
    let readback = rebuilt
        .complete_nearai_wallet_login(caller(&f.service.db_tenant_id), signed.clone())
        .await
        .unwrap();
    assert_eq!(readback.mutation_receipt, result.mutation_receipt);
    assert_eq!(reference(&f).await, current);
    assert_eq!(f.wallet_peer.requests.lock().await.len(), 1);
    let mut foreign = signed.clone();
    foreign.account_id = "bob.near".into();
    assert!(matches!(
        finish(&f, foreign).await,
        Err(LlmConfigServiceError::Conflict { .. })
    ));
    assert_eq!(f.wallet_peer.requests.lock().await.len(), 1);
    file_unchanged(&f);
    let client = f.service.pg_pool.get().await.unwrap();
    let metadata:serde_json::Value=client.query_one("SELECT request FROM brassclaw_llm_provider_mutations WHERE tenant_id=$1 AND mutation_id=$2",&[&f.service.db_tenant_id,&challenge.mutation_id]).await.unwrap().get(0);
    assert!(!metadata.to_string().contains(SIGNATURE));
    assert!(!metadata.to_string().contains("wallet-issued-token"));
    drop(client);
    transport.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_wallet_exchange_is_one_shot_and_settings_remain_editable_during_wait() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let f = fixture(rig.pool.clone(), "native-wallet-wait").await;
    f.service
        .keys
        .put("nearai", SecretMaterial::from("retained-provider-key"))
        .await
        .unwrap();
    f.wallet_peer.mode.store(1, Ordering::SeqCst);
    let challenge = start(&f).await;
    let signed = response(&challenge);
    let service = f.service.clone();
    let request = signed.clone();
    let operator = caller(&f.service.db_tenant_id);
    let submitted = tokio::spawn(async move {
        service
            .complete_nearai_wallet_login(operator, request)
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), f.wallet_peer.entered.notified())
        .await
        .unwrap();
    assert!(matches!(
        finish(&f, signed.clone()).await,
        Err(LlmConfigServiceError::Conflict { .. })
    ));
    assert_eq!(f.wallet_peer.requests.lock().await.len(), 1);
    let snapshot = f
        .service
        .snapshot(caller(&f.service.db_tenant_id))
        .await
        .unwrap();
    let revision = snapshot
        .providers
        .iter()
        .find(|row| row.id == "nearai")
        .unwrap()
        .revision;
    let edited=tokio::time::timeout(Duration::from_secs(3),f.service.set_active(caller(&f.service.db_tenant_id),serde_json::from_value(json!({
        "mutation_id":uuid::Uuid::new_v4(),"provider_id":"nearai","expected_revision":revision,
        "expected_selection_ref":snapshot.role_selection_refs["kohai"],"model":"later-operator-model","role":"kohai"})).unwrap())).await.unwrap().unwrap();
    assert_eq!(
        edited.kohai_active.as_ref().unwrap().model.as_deref(),
        Some("later-operator-model")
    );
    let retained = reference(&f).await;
    f.wallet_peer.release.notify_one();
    assert!(matches!(
        submitted.await.unwrap(),
        Err(LlmConfigServiceError::Conflict { .. })
    ));
    assert_eq!(reference(&f).await, retained);
    assert!(finish(&f, signed).await.is_err());
    assert_eq!(f.wallet_peer.requests.lock().await.len(), 1);
    // The selected HTTP deadline is adjustable provider data, not another fixed timeout.
    let mut definition = f
        .service
        .pg_provider_repo
        .get_revision("nearai", false)
        .await
        .unwrap()
        .unwrap()
        .definition;
    definition.request_policy.timeout_secs = 1;
    f.service.pg_provider_repo.upsert(definition).await.unwrap();
    let next = start(&f).await;
    let request = response(&next);
    let begun = std::time::Instant::now();
    assert!(finish(&f, request.clone()).await.is_err());
    assert!(begun.elapsed() < Duration::from_millis(2500));
    assert_eq!(f.wallet_peer.requests.lock().await.len(), 2);
    assert!(finish(&f, request).await.is_err());
    assert_eq!(f.wallet_peer.requests.lock().await.len(), 2);
    f.wallet_peer.release.notify_one();
    // Unknown state after restart cannot turn a still-valid signature into another call.
    let lost = start(&f).await;
    f.service.nearai_login_states.states.lock().await.clear();
    assert!(finish(&f, response(&lost)).await.is_err());
    assert_eq!(f.wallet_peer.requests.lock().await.len(), 2);
    file_unchanged(&f);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_wallet_rejects_mismatched_challenges_and_private_or_redirected_replies() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let f = fixture(rig.pool.clone(), "native-wallet-rejections").await;
    f.service
        .keys
        .put("nearai", SecretMaterial::from("original-provider-key"))
        .await
        .unwrap();
    let original = reference(&f).await;
    for mode in [2, 3, 5, 6] {
        let mut definition = f
            .service
            .pg_provider_repo
            .get_revision("nearai", false)
            .await
            .unwrap()
            .unwrap()
            .definition;
        definition.request_policy.max_response_bytes = (mode == 6).then_some(64);
        f.service.pg_provider_repo.upsert(definition).await.unwrap();
        f.wallet_peer.mode.store(mode, Ordering::SeqCst);
        let challenge = start(&f).await;
        let signed = response(&challenge);
        let before = f.wallet_peer.requests.lock().await.len();
        let mut different = signed.clone();
        different.nonce[31] ^= 1;
        assert!(finish(&f, different).await.is_err());
        assert_eq!(f.wallet_peer.requests.lock().await.len(), before);
        let error = finish(&f, signed.clone()).await.unwrap_err().to_string();
        assert!(!error.contains("private-wallet-error"));
        assert!(!error.contains("private-response-token"));
        assert_eq!(f.wallet_peer.requests.lock().await.len(), before + 1);
        assert!(finish(&f, signed).await.is_err());
        assert_eq!(f.wallet_peer.requests.lock().await.len(), before + 1);
        assert_eq!(reference(&f).await, original);
        assert_eq!(f.wallet_peer.redirected.load(Ordering::SeqCst), 0);
        file_unchanged(&f);
    }
    let challenge = start(&f).await;
    let mut invalid = response(&challenge);
    invalid.signature.clear();
    assert!(matches!(
        finish(&f, invalid).await,
        Err(LlmConfigServiceError::InvalidRequest { .. })
    ));
    let client = f.service.pg_pool.get().await.unwrap();
    let records: i64 = client
        .query_one(
            "SELECT count(*) FROM brassclaw_llm_provider_mutations WHERE tenant_id=$1",
            &[&f.service.db_tenant_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(records, 0);
    assert!(f.model_auth.lock().await.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_wallet_fences_expired_cross_protocol_and_pre_exchange_settings() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let f = fixture(rig.pool.clone(), "native-wallet-pre-exchange").await;
    for case in ["expired", "definition", "credential"] {
        let challenge = start(&f).await;
        let state = challenge.mutation_id.to_string();
        // The public browser callback must not consume a wallet challenge.
        assert!(
            f.service
                .complete_nearai_browser_login(&state, "foreign-browser-token")
                .await
                .is_err()
        );
        assert!(
            f.service
                .nearai_login_states
                .states
                .lock()
                .await
                .contains_key(&state)
        );
        match case {
            "expired" => {
                f.service
                    .nearai_login_states
                    .states
                    .lock()
                    .await
                    .get_mut(&state)
                    .unwrap()
                    .0 = Instant::now() - Duration::from_secs(1);
            }
            "definition" => {
                let mut definition = f
                    .service
                    .pg_provider_repo
                    .get_revision("nearai", false)
                    .await
                    .unwrap()
                    .unwrap()
                    .definition;
                definition
                    .description
                    .push_str(" changed before signing completed");
                f.service.pg_provider_repo.upsert(definition).await.unwrap();
            }
            "credential" => f
                .service
                .keys
                .put("nearai", SecretMaterial::from("later-operator-key"))
                .await
                .unwrap(),
            _ => unreachable!(),
        }
        let retained = reference(&f).await;
        assert!(finish(&f, response(&challenge)).await.is_err(), "{case}");
        assert!(
            f.wallet_peer.requests.lock().await.is_empty(),
            "{case}: no stale auth egress"
        );
        assert_eq!(reference(&f).await, retained);
        file_unchanged(&f);
    }
}
