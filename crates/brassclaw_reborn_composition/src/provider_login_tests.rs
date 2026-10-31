//! Actual provider service, encrypted PostgreSQL, public HTTP callback, live SDK
//! and COMMIT acknowledgement loss. Controlled HTTP replies are not model quality.
use super::*;
use brassclaw_llm::{ChatMessage, CompletionRequest, LlmProvider, SwappableLlmProvider};
use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
use brassclaw_secrets::SecretMaterial;

pub(super) struct Fixture {
    pub(super) service: Arc<RebornLlmConfigService>,
    pub(super) primary: Arc<SwappableLlmProvider>,
    pub(super) directory: tempfile::TempDir,
    pub(super) original_file: Vec<u8>,
    pub(super) model_auth: Arc<tokio::sync::Mutex<Vec<String>>>,
    pub(super) wallet_peer: Arc<WalletPeer>,
    model_names: Arc<tokio::sync::Mutex<Vec<String>>>,
    _stop: tokio_util::sync::CancellationToken,
    _server: tokio_util::task::AbortOnDropHandle<()>,
}
#[derive(Default)]
pub(super) struct WalletPeer {
    pub(super) requests: tokio::sync::Mutex<Vec<serde_json::Value>>,
    pub(super) mode: std::sync::atomic::AtomicU8,
    pub(super) entered: tokio::sync::Notify,
    pub(super) release: tokio::sync::Notify,
    pub(super) redirected: std::sync::atomic::AtomicUsize,
}
pub(super) async fn fixture(pool: Arc<brassclaw_pg::PgPool>, tenant: &str) -> Fixture {
    use axum::{
        Json, Router,
        extract::State,
        http::HeaderMap,
        routing::{get, post},
    };
    let model_auth: Arc<tokio::sync::Mutex<Vec<String>>> = Default::default();
    let model_names: Arc<tokio::sync::Mutex<Vec<String>>> = Default::default();
    let observed_models = model_names.clone();
    let router = Router::new()
        .route("/v1/model/list", get(|| async { Json(json!({"models":[]})) }))
        .route("/v1/chat/completions", post(move |State(log): State<Arc<tokio::sync::Mutex<Vec<String>>>>, headers: HeaderMap, Json(body): Json<serde_json::Value>| {
            let observed_models = observed_models.clone();
            async move {
                log.lock().await.push(headers["authorization"].to_str().unwrap().into());
                observed_models.lock().await.push(body["model"].as_str().unwrap().into());
                Json(json!({"id":"actual-login-response","object":"chat.completion","created":0,"model":"login-model",
                    "choices":[{"index":0,"message":{"role":"assistant","content":"login-account-response"},"finish_reason":"stop"}],
                    "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}))
            }
        })).with_state(model_auth.clone());
    let wallet_peer = Arc::new(WalletPeer::default());
    let wallet_router=Router::new().route("/v1/auth/near",post(|State(peer):State<Arc<WalletPeer>>,Json(body):Json<serde_json::Value>|async move {
        use axum::response::IntoResponse;
        let mode=peer.mode.load(std::sync::atomic::Ordering::SeqCst);
        peer.requests.lock().await.push(body);
        peer.entered.notify_one();
        if mode==1 {peer.release.notified().await;}
        match mode {
            2=>(axum::http::StatusCode::INTERNAL_SERVER_ERROR,Json(json!({"error":"private-wallet-error","access_token":"private-response-token"}))).into_response(),
            3=>Json(json!({"access_token":{"private":"private-response-token"}})).into_response(),
            5=>axum::response::Response::builder().status(307).header("location","/must-not-follow").body(axum::body::Body::empty()).unwrap(),
            6=>Json(json!({"access_token":"x".repeat(1024)})).into_response(),
            _=>Json(json!({"access_token":"wallet-issued-token"})).into_response(),
        }
    })).route("/must-not-follow",axum::routing::any(|State(peer):State<Arc<WalletPeer>>|async move {
        peer.redirected.fetch_add(1,std::sync::atomic::Ordering::SeqCst);
        Json(json!({"access_token":"redirected-token"}))
    })).with_state(wallet_peer.clone());
    let (address, stop, server) =
        crate::test_loopback_http::spawn(router.merge(wallet_router)).await;
    let endpoint = format!("http://{address}");
    let repo = Arc::new(crate::PgProviderRepo::new(pool.as_ref().clone(), tenant));
    repo.initialize().await.unwrap();
    repo.upsert(serde_json::from_value(json!({"id":"nearai","protocol":"nearai","default_base_url":endpoint,
        "default_model":"login-model","api_key_required":false,"model_env":"UNUSED_LOGIN_MODEL","description":"Controlled NEAR login fixture"})).unwrap()).await.unwrap();
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            pool.as_ref().clone(),
            "controlled-nearai-login-master-key".into(),
            tenant,
        )
        .unwrap(),
    ));
    let directory = tempfile::tempdir().unwrap();
    let session_path = directory.path().join("legacy-session.json");
    let original_file = serde_json::to_vec(&brassclaw_llm::session::SessionData {
        session_token: "legacy-file-token".into(),
        created_at: chrono::Utc::now(),
        auth_provider: Some("legacy".into()),
    })
    .unwrap();
    std::fs::write(&session_path, &original_file).unwrap();
    let session = Arc::new(brassclaw_llm::SessionManager::new(
        brassclaw_llm::SessionConfig {
            auth_base_url: endpoint,
            session_path,
        },
    ));
    let primary = Arc::new(SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let reload = Arc::new(crate::llm_reload::RebornLlmReloadAdapter::new(
        Arc::new(brassclaw_llm::LlmReloadHandle::new(primary.clone(), None)),
        session.clone(),
        keys.clone(),
        pool.clone(),
        tenant,
    ));
    let service = Arc::new(
        RebornLlmConfigService::new(keys, pool, repo, tenant)
            .with_nearai_session(session)
            .with_reload_trigger(reload),
    );
    Fixture {
        service,
        primary,
        directory,
        original_file,
        model_auth,
        wallet_peer,
        model_names,
        _stop: stop,
        _server: server,
    }
}
pub(super) fn caller(tenant: &str) -> WebUiAuthenticatedCaller {
    WebUiAuthenticatedCaller::new(
        brassclaw_host_api::TenantId::new(tenant).unwrap(),
        brassclaw_host_api::UserId::new("login-operator").unwrap(),
        None,
        None,
    )
}
async fn start(f: &Fixture, origin: &str) -> brassclaw_product_workflow::NearAiLoginStart {
    f.service
        .start_nearai_login(
            caller(&f.service.db_tenant_id),
            serde_json::from_value(json!({"provider":"github","origin":origin})).unwrap(),
        )
        .await
        .unwrap()
}
pub(super) async fn reference(f: &Fixture) -> Option<String> {
    let mut client = f.service.pg_pool.get().await.unwrap();
    let tx = client.transaction().await.unwrap();
    let reference = f
        .service
        .keys
        .inspect_record_reference_in_transaction(&tx, "nearai")
        .await
        .unwrap();
    tx.commit().await.unwrap();
    reference
}
pub(super) fn file_unchanged(f: &Fixture) {
    assert_eq!(
        std::fs::read(f.directory.path().join("legacy-session.json")).unwrap(),
        f.original_file
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_nearai_browser_callback_keeps_original_through_ack_loss_and_restart_readback() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![b"upsert".to_vec(), BROWSER_LOGIN.as_bytes().to_vec()],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    let f = fixture(transport.pool.clone(), "native-nearai-original").await;
    let mount = crate::nearai_login_serve::nearai_login_callback_mount(f.service.clone());
    let (callback, stop, server) = crate::test_loopback_http::spawn(mount.router).await;
    let login = start(&f, &format!("http://{callback}")).await;
    let auth = url::Url::parse(&login.auth_url).unwrap();
    let original_callback = auth
        .query_pairs()
        .find(|(key, _)| key == "frontend_callback")
        .unwrap()
        .1
        .into_owned();
    assert!(original_callback.ends_with(&login.mutation_id.to_string()));
    let url = format!("{original_callback}/auth/callback?token=explicit-browser-token");
    transport.arm(1).unwrap();
    let http_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let response = http_client.get(&url).send().await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER);
    assert_eq!(response.headers()["location"], "/v2/chat");
    assert_eq!(transport.cut_count(), 1);
    let key = f.service.keys.read("nearai").await.unwrap().unwrap();
    assert_eq!(key.expose_secret(), "explicit-browser-token");
    file_unchanged(&f);
    let before = reference(&f).await;
    assert_eq!(
        f.primary
            .complete(CompletionRequest::new(vec![ChatMessage::user(
                "authenticated native caller"
            )]))
            .await
            .unwrap()
            .content,
        "login-account-response"
    );
    assert_eq!(
        &*f.model_auth.lock().await,
        &["Bearer explicit-browser-token"]
    );
    let client = f.service.pg_pool.get().await.unwrap();
    let recorded:serde_json::Value=client.query_one("SELECT request FROM brassclaw_llm_provider_mutations WHERE tenant_id=$1 AND mutation_id=$2",&[&f.service.db_tenant_id,&login.mutation_id]).await.unwrap().get(0);
    assert!(
        !recorded.to_string().contains("explicit-browser-token"),
        "credential value must remain in the encrypted host store"
    );
    drop(client);
    let receipt = f
        .service
        .provider_mutation(caller(&f.service.db_tenant_id), login.mutation_id)
        .await
        .unwrap();
    assert_eq!(receipt.provider_id.as_deref(), Some("nearai"));
    assert_eq!(receipt.operation, "upsert");
    for _ in 0..2 {
        // Empty RAM state also represents a process restart at this boundary;
        // only the exact committed original may be reconciled, never reapplied.
        f.service.nearai_login_states.states.lock().await.clear();
        let repeated = http_client.get(&url).send().await.unwrap();
        assert_eq!(repeated.headers()["location"], "/v2/chat");
        assert_eq!(reference(&f).await, before);
        assert_eq!(
            f.service
                .provider_mutation(caller(&f.service.db_tenant_id), login.mutation_id)
                .await
                .unwrap(),
            receipt
        );
    }
    let rebuilt = RebornLlmConfigService::new(
        f.service.keys.clone(),
        f.service.pg_pool.clone(),
        f.service.pg_provider_repo.clone(),
        &f.service.db_tenant_id,
    )
    .with_nearai_session(f.service.nearai_session.clone().unwrap())
    .with_reload_trigger(f.service.reload.clone().unwrap());
    let readback = rebuilt
        .complete_nearai_browser_login(&login.mutation_id.to_string(), "explicit-browser-token")
        .await
        .unwrap();
    assert_eq!(readback.mutation_receipt.as_ref(), Some(&receipt));
    assert_eq!(
        reference(&f).await,
        before,
        "a rebuilt configuration owner only reads the committed original"
    );
    assert!(
        f.service
            .complete_nearai_browser_login(&login.mutation_id.to_string(), "different-token")
            .await
            .is_err()
    );
    assert_eq!(reference(&f).await, before);
    file_unchanged(&f);
    f.service
        .keys
        .put("nearai", SecretMaterial::from("later-current-token"))
        .await
        .unwrap();
    let successor = reference(&f).await;
    assert_ne!(successor, before);
    let settled = f
        .service
        .complete_nearai_browser_login(&login.mutation_id.to_string(), "explicit-browser-token")
        .await
        .unwrap();
    assert_eq!(settled.mutation_receipt.as_ref(), Some(&receipt));
    assert_eq!(
        reference(&f).await,
        successor,
        "reading an old receipt never restores its credentials over a later edit"
    );
    f.primary
        .complete(CompletionRequest::new(vec![ChatMessage::user(
            "current explicit exchange",
        )]))
        .await
        .unwrap();
    assert_eq!(
        &*f.model_auth.lock().await,
        &[
            "Bearer explicit-browser-token",
            "Bearer later-current-token"
        ]
    );
    file_unchanged(&f);
    stop.cancel();
    server.await.unwrap();
    transport.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_nearai_browser_stale_role_credential_definition_and_rejected_commit_have_no_publication()
 {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let f = fixture(rig.pool.clone(), "native-nearai-conflicts").await;
    for case in [
        "role",
        "credential",
        "definition",
        "commit-rejection",
        "expired",
    ] {
        let login = start(&f, "http://127.0.0.1:19999").await;
        match case {
            "role"=>crate::db_config::save_config_key(&f.service.pg_pool,&f.service.db_tenant_id,"llm.default.model","later-operator-model",crate::db_config::ConfigWriteContext::Operator).await.unwrap(),
            "credential"=>f.service.keys.put("nearai",SecretMaterial::from("later-operator-token")).await.unwrap(),
            "definition"=>{let mut definition=f.service.pg_provider_repo.get_revision("nearai",false).await.unwrap().unwrap().definition;definition.description.push_str(" revised");f.service.pg_provider_repo.upsert(definition).await.unwrap();},
            "commit-rejection"=>f.service.pg_pool.get().await.unwrap().batch_execute("CREATE FUNCTION reject_login_receipt() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.request->>'kind'='nearai-browser-login/1' THEN RAISE EXCEPTION 'controlled receipt rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_login_receipt BEFORE INSERT ON brassclaw_llm_provider_mutations FOR EACH ROW EXECUTE FUNCTION reject_login_receipt();").await.unwrap(),
            "expired"=>{f.service.nearai_login_states.states.lock().await.get_mut(&login.mutation_id.to_string()).unwrap().0=Instant::now()-Duration::from_secs(1);},
            _=>unreachable!(),
        }
        let before = reference(&f).await;
        let configuration =
            crate::db_config::list_config_keys(&f.service.pg_pool, &f.service.db_tenant_id)
                .await
                .unwrap();
        assert!(
            f.service
                .complete_nearai_browser_login(
                    &login.mutation_id.to_string(),
                    "stale-browser-token"
                )
                .await
                .is_err(),
            "{case}"
        );
        assert_eq!(
            reference(&f).await,
            before,
            "{case}: no credential overwrite"
        );
        assert_eq!(
            crate::db_config::list_config_keys(&f.service.pg_pool, &f.service.db_tenant_id)
                .await
                .unwrap(),
            configuration,
            "{case}: no partial selection"
        );
        assert!(matches!(
            f.service
                .provider_mutation(caller(&f.service.db_tenant_id), login.mutation_id)
                .await,
            Err(LlmConfigServiceError::NotFound)
        ));
        assert!(f.model_auth.lock().await.is_empty());
        file_unchanged(&f);
        if case == "commit-rejection" {
            f.service.pg_pool.get().await.unwrap().batch_execute("DROP TRIGGER reject_login_receipt ON brassclaw_llm_provider_mutations; DROP FUNCTION reject_login_receipt()").await.unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_nearai_login_competing_credential_compare_writes_retain_one_winner() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let f = fixture(rig.pool.clone(), "native-nearai-key-cas").await;
    for expected in [None, Some("initial-present")] {
        if expected.is_some() {
            f.service
                .keys
                .put("nearai", SecretMaterial::from("initial-present"))
                .await
                .unwrap();
        }
        let original = reference(&f).await;
        let write = |value: &'static str| {
            let f = &f;
            let original = original.clone();
            async move {
                let mut client = f.service.pg_pool.get().await.unwrap();
                let transaction = client.transaction().await.unwrap();
                let applied = f
                    .service
                    .keys
                    .put_if_record_reference_in_transaction(
                        &transaction,
                        "nearai",
                        SecretMaterial::from(value),
                        original.as_deref(),
                    )
                    .await
                    .unwrap();
                transaction.commit().await.unwrap();
                applied
            }
        };
        let (a, b) = tokio::join!(write("winner-a"), write("winner-b"));
        assert_ne!(a, b, "exactly one original may replace this record");
        let value = f.service.keys.read("nearai").await.unwrap().unwrap();
        assert_eq!(
            value.expose_secret(),
            if a { "winner-a" } else { "winner-b" }
        );
    }
    let before = reference(&f).await;
    let wrong = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            f.service.pg_pool.as_ref().clone(),
            "different-controlled-nearai-login-master-key".into(),
            &f.service.db_tenant_id,
        )
        .unwrap(),
    ));
    let mut client = f.service.pg_pool.get().await.unwrap();
    let transaction = client.transaction().await.unwrap();
    assert!(
        wrong
            .put_if_record_reference_in_transaction(
                &transaction,
                "nearai",
                SecretMaterial::from("must-not-overwrite"),
                before.as_deref()
            )
            .await
            .is_err()
    );
    transaction.rollback().await.unwrap();
    assert_eq!(reference(&f).await, before);
    file_unchanged(&f);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_nearai_logins_preserve_selected_model_without_copying_foreign_model() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    for (channel, same_provider) in [
        ("browser", true),
        ("wallet", true),
        ("browser", false),
        ("wallet", false),
    ] {
        let f = fixture(
            rig.pool.clone(),
            &format!("native-login-model-{channel}-{same_provider}"),
        )
        .await;
        if !same_provider {
            let mut other = f
                .service
                .pg_provider_repo
                .get_revision("nearai", false)
                .await
                .unwrap()
                .unwrap()
                .definition;
            other.id = "other-provider".into();
            other.default_model = "foreign-model".into();
            f.service.pg_provider_repo.upsert(other).await.unwrap();
        }
        crate::db_config::save_config_changes(
            &f.service.pg_pool,
            &f.service.db_tenant_id,
            &[
                (
                    "llm.default.provider_id",
                    Some(if same_provider {
                        "nearai"
                    } else {
                        "other-provider"
                    }),
                ),
                (
                    "llm.default.model",
                    Some(if same_provider {
                        "selected-custom-model"
                    } else {
                        "foreign-model"
                    }),
                ),
            ],
            crate::db_config::ConfigWriteContext::Operator,
        )
        .await
        .unwrap();
        if channel == "browser" {
            let login = start(&f, "http://127.0.0.1:19999").await;
            f.service
                .complete_nearai_browser_login(
                    &login.mutation_id.to_string(),
                    "model-preserved-browser-token",
                )
                .await
                .unwrap();
        } else {
            let challenge = f
                .service
                .start_nearai_wallet_login(caller(&f.service.db_tenant_id), Default::default())
                .await
                .unwrap();
            f.service
                .complete_nearai_wallet_login(
                    caller(&f.service.db_tenant_id),
                    NearAiWalletLoginRequest {
                        mutation_id: challenge.mutation_id,
                        account_id: "alice.near".into(),
                        public_key: "ed25519:fixture".into(),
                        signature: "controlled-wallet-response".into(),
                        message: challenge.message,
                        recipient: challenge.recipient,
                        nonce: challenge.nonce,
                        callback_url: challenge.callback_url,
                    },
                )
                .await
                .unwrap();
        }
        let expected = if same_provider {
            "selected-custom-model"
        } else {
            "login-model"
        };
        let snapshot = f
            .service
            .snapshot(caller(&f.service.db_tenant_id))
            .await
            .unwrap();
        assert_eq!(
            snapshot.kohai_active.as_ref().unwrap().model.as_deref(),
            same_provider.then_some("selected-custom-model")
        );
        f.primary
            .complete(CompletionRequest::new(vec![ChatMessage::user(
                "actual retained login model",
            )]))
            .await
            .unwrap();
        assert_eq!(&*f.model_names.lock().await, &[expected]);
        file_unchanged(&f);
    }
}
