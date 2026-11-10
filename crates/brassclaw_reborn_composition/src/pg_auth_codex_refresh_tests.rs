//! Native PG/HTTP refresh acceptance through the production admission, original
//! custody and publisher. The parent's local endpoint adapter does not qualify
//! production DNS/TLS/private-IP policy or actual OpenAI compatibility.
use super::*;
use crate::pg_auth_product_services::codex_refresh::CodexAccessSelection;

async fn published_login(
    rig: &crate::runtime::test_pg::native_pg::NativePostgres,
    auth: &PgAuthProductServices,
    issuer: &Issuer,
) -> (
    crate::oauth_provider_client::HostOAuthProviderClient,
    String,
    CredentialAccount,
) {
    published_login_on_pool(&rig.pool, auth, issuer).await
}

async fn published_login_on_pool(
    pool: &Arc<brassclaw_pg::PgPool>,
    auth: &PgAuthProductServices,
    issuer: &Issuer,
) -> (
    crate::oauth_provider_client::HostOAuthProviderClient,
    String,
    CredentialAccount,
) {
    let transport = transport(pool, auth, issuer, None, None).await;
    let original = prepare(auth, &transport, 60).await.original;
    let challenge = step(auth, &transport, &original).await;
    wait_until_due(&challenge).await;
    step(auth, &transport, &original).await;
    step(auth, &transport, &original).await;
    let repo = crate::PgProviderRepo::new(pool.as_ref().clone(), "native-device-test");
    repo.initialize().await.unwrap();
    let revision = repo
        .get_revision("openai_codex", false)
        .await
        .unwrap()
        .unwrap()
        .revision;
    let mut connection = pool.get().await.unwrap();
    let tx = connection.transaction().await.unwrap();
    let ready = auth
        .native_device_ready_in_transaction(&tx, &original)
        .await
        .unwrap();
    let published = auth
        .publish_codex_account_and_head_in_transaction(
            &tx,
            &ready,
            "native-device-test",
            "openai_codex",
            None,
        )
        .await
        .unwrap();
    let mutation=crate::provider_mutation::ProviderMutation::new(original.definition.flow_id.as_uuid(),"upsert",
        serde_json::json!({"kind":crate::provider_codex_credentials::LOGIN_KIND,"credential_binding":published.binding}),None).unwrap();
    mutation
        .record_with_links(
            &tx,
            "native-device-test",
            "actual-test-installer",
            Some(("openai_codex", revision)),
            &crate::LlmKeyStore::new_postgres(auth.secret_store.clone()),
            crate::provider_mutation::ProviderMutationLinks {
                bootstrap_job_id: None,
                credential_head_ref: Some(published.head_reference.clone()),
            },
        )
        .await
        .unwrap();
    auth.finish_native_device_in_transaction(&tx, &ready, &published.account)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    (transport, published.head_reference, published.account)
}

#[cfg(feature = "skills-db")]
#[path = "pg_auth_codex_product_tests.rs"]
mod product_tests;

#[tokio::test]
async fn native_codex_refresh_two_owners_share_one_generation_and_omitted_refresh_is_retained() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let issuer = Issuer::start(IssuerMode::RefreshOmit).await;
    let (transport, head, account) = published_login(&rig, &auth, &issuer).await;
    let selected = CodexAccessSelection {
        namespace: "native-device-test",
        provider_id: "openai_codex",
        head: &head,
        api_base_url: "https://native-device.example/backend-api",
        margin_secs: 3600,
    };
    let other = service(&rig.pool);
    let (a, b) = tokio::join!(
        auth.admit_codex_refresh(&selected),
        other.admit_codex_refresh(&selected)
    );
    let (a, b) = (a.unwrap().unwrap(), b.unwrap().unwrap());
    // Both public callers use the sealed production admissions. Exactly one
    // admission may transport; the recovered one performs readback only.
    let (first, second) = tokio::join!(
        auth.execute_codex_refresh(&transport, &a, &selected),
        other.execute_codex_refresh(&transport, &b, &selected)
    );
    assert!(
        first.is_ok() || second.is_ok(),
        "the winning original publishes its actual return"
    );
    assert_eq!(
        issuer.state.token_calls.load(Ordering::SeqCst),
        2,
        "one login exchange plus one refresh"
    );
    let (after, _) = auth
        .read_account(&account.scope, account.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.refresh_secret, account.refresh_secret);
    assert_ne!(after.access_secret, account.access_secret);
    assert_eq!(after.status, CredentialAccountStatus::Configured);
    let rows = rig
        .pool
        .get()
        .await
        .unwrap()
        .query(
            "SELECT data FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1",
            &[&"native-device-test"],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let action: brassclaw_auth::CredentialRefreshAction =
        serde_json::from_value(rows[0].get(0)).unwrap();
    assert!(action.receipt().unwrap().confirmed_refresh());
    assert!(
        !serde_json::to_string(&action.receipt().unwrap())
            .unwrap()
            .contains("refresh-http-original")
    );
    // No replay of an old sealed dispatch is possible after publication.
    assert!(
        auth.execute_codex_refresh(&transport, &a, &selected)
            .await
            .is_err()
    );
    assert_eq!(issuer.state.token_calls.load(Ordering::SeqCst), 2);
    // Force preparation within the selected supported margin for the actually
    // new access generation. The unchanged refresh token must not key it back
    // to the preceding action.
    let next = other.admit_codex_refresh(&selected).await.unwrap().unwrap();
    other
        .execute_codex_refresh(&transport, &next, &selected)
        .await
        .unwrap();
    assert_eq!(issuer.state.token_calls.load(Ordering::SeqCst), 3);
    let count: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1",
            &[&"native-device-test"],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 2);
}

#[tokio::test]
async fn native_codex_refresh_invalid_grant_and_lost_return_never_redispatch_after_restart() {
    for mode in [IssuerMode::RefreshInvalid, IssuerMode::RefreshDisconnect] {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let auth = service(&rig.pool);
        let issuer = Issuer::start(mode).await;
        let (transport, head, _) = published_login(&rig, &auth, &issuer).await;
        let selected = CodexAccessSelection {
            namespace: "native-device-test",
            provider_id: "openai_codex",
            head: &head,
            api_base_url: "https://native-device.example/backend-api",
            margin_secs: 3600,
        };
        let admitted = auth.admit_codex_refresh(&selected).await.unwrap().unwrap();
        assert!(
            auth.execute_codex_refresh(&transport, &admitted, &selected)
                .await
                .is_err()
        );
        let reopened = service(&rig.pool);
        if let Ok(Some(original)) = reopened.admit_codex_refresh(&selected).await {
            assert!(
                reopened
                    .execute_codex_refresh(&transport, &original, &selected)
                    .await
                    .is_err()
            );
        }
        assert_eq!(issuer.state.token_calls.load(Ordering::SeqCst), 2);
        let count: i64 = rig
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1",
                &[&"native-device-test"],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(count, 1);
    }
}

#[tokio::test]
async fn native_codex_refresh_retained_response_reconciles_failed_publication_without_issuer_replay()
 {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let issuer = Issuer::start(IssuerMode::Success).await;
    let (transport, head, account) = published_login(&rig, &auth, &issuer).await;
    let selected = CodexAccessSelection {
        namespace: "native-device-test",
        provider_id: "openai_codex",
        head: &head,
        api_base_url: "https://native-device.example/backend-api",
        margin_secs: 3600,
    };
    rig.pool.get().await.unwrap().batch_execute("CREATE FUNCTION reject_refresh_publication() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'controlled actual publication failure'; END $$; CREATE TRIGGER reject_refresh_publication BEFORE UPDATE ON brassclaw_product_auth_accounts FOR EACH ROW EXECUTE FUNCTION reject_refresh_publication()").await.unwrap();
    let admitted = auth.admit_codex_refresh(&selected).await.unwrap().unwrap();
    assert!(
        auth.execute_codex_refresh(&transport, &admitted, &selected)
            .await
            .is_err()
    );
    assert_eq!(issuer.state.token_calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        auth.read_account(&account.scope, account.id)
            .await
            .unwrap()
            .unwrap()
            .0,
        account
    );
    rig.pool.get().await.unwrap().batch_execute("DROP TRIGGER reject_refresh_publication ON brassclaw_product_auth_accounts; DROP FUNCTION reject_refresh_publication()").await.unwrap();
    let reopened = service(&rig.pool);
    let original = reopened
        .admit_codex_refresh(&selected)
        .await
        .unwrap()
        .unwrap();
    reopened
        .execute_codex_refresh(&transport, &original, &selected)
        .await
        .unwrap();
    assert_eq!(issuer.state.token_calls.load(Ordering::SeqCst), 2);
    assert_ne!(
        reopened
            .read_account(&account.scope, account.id)
            .await
            .unwrap()
            .unwrap()
            .0
            .access_secret,
        account.access_secret
    );
}

#[tokio::test]
async fn native_codex_refresh_confirmed_no_send_keeps_account_and_original_stop_after_restart() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let issuer = Issuer::start(IssuerMode::Success).await;
    let (transport, head, account) = published_login(&rig, &auth, &issuer).await;
    let unavailable = transport.with_original_egress(None);
    let selected = CodexAccessSelection {
        namespace: "native-device-test",
        provider_id: "openai_codex",
        head: &head,
        api_base_url: "https://native-device.example/backend-api",
        margin_secs: 3600,
    };
    let admitted = auth.admit_codex_refresh(&selected).await.unwrap().unwrap();
    assert!(
        auth.execute_codex_refresh(&unavailable, &admitted, &selected)
            .await
            .is_err()
    );
    assert_eq!(issuer.state.token_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        auth.read_account(&account.scope, account.id)
            .await
            .unwrap()
            .unwrap()
            .0,
        account
    );
    let row = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT data FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1",
            &[&"native-device-test"],
        )
        .await
        .unwrap();
    let action: brassclaw_auth::CredentialRefreshAction =
        serde_json::from_value(row.get(0)).unwrap();
    assert_eq!(
        action.receipt().unwrap().outcome,
        brassclaw_auth::CredentialRefreshOutcomeKind::NotSubmitted
    );
    let restarted = service(&rig.pool);
    let original = restarted
        .admit_codex_refresh(&selected)
        .await
        .unwrap()
        .unwrap();
    assert!(
        restarted
            .execute_codex_refresh(&unavailable, &original, &selected)
            .await
            .is_err()
    );
    assert_eq!(issuer.state.token_calls.load(Ordering::SeqCst), 1);
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RefreshWorkerInput {
    url: String,
    endpoint: String,
    head: String,
    barrier: std::path::PathBuf,
    name: String,
}

#[tokio::test]
#[ignore = "private cross-process worker started only by the parent PG/HTTP fixture"]
async fn native_codex_refresh_crossprocess_worker() {
    assert_eq!(
        std::env::var("BRASSCLAW_CODEX_REFRESH_WORKER").expect("private worker fixture"),
        "1"
    );
    // Test-only process input over a private pipe, never a command-line URL or
    // a product DTO. No token or private response is sent between processes.
    let input: RefreshWorkerInput = serde_json::from_reader(std::io::stdin()).unwrap();
    let pool = Arc::new(brassclaw_pg::pool::build_pool(&input.url).unwrap());
    let auth = service(&pool);
    let settings = Arc::new(crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
        pool.clone(),
        "native-device-test",
        "default",
    ));
    let current = settings.get("default", "default").await.unwrap();
    let source = Arc::new(crate::host_primitive_settings::AcknowledgedPrimitiveSettings::new());
    source
        .prepare_bootstrap(brassclaw_host_runtime::HostPrimitiveSettings {
            revision: current.revision,
            limits: current.execution_limits,
        })
        .unwrap();
    source.bind(settings).await.unwrap();
    let transport = endpoint_transport(&auth, &input.endpoint, source);
    let selected = CodexAccessSelection {
        namespace: "native-device-test",
        provider_id: "openai_codex",
        head: &input.head,
        api_base_url: "https://native-device.example/backend-api",
        margin_secs: 3600,
    };
    let admitted = auth.admit_codex_refresh(&selected).await.unwrap().unwrap();
    std::fs::write(input.barrier.join(&input.name), b"admitted").unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        while !input.barrier.join("release").exists() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    // An original observer may still be pending while the winner transports.
    // The parent verifies one actual refresh and its confirmed native receipt.
    let _outcome = auth
        .execute_codex_refresh(&transport, &admitted, &selected)
        .await;
    pool.close();
}

#[tokio::test]
async fn native_codex_two_processes_refresh_one_original_generation_once() {
    use tokio::io::AsyncWriteExt;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let issuer = Issuer::start(IssuerMode::Success).await;
    let (_, head, _) = published_login(&rig, &auth, &issuer).await;
    let barrier = tempfile::tempdir().unwrap();
    let filter = format!(
        "{}::native_codex_refresh_crossprocess_worker",
        module_path!().split_once("::").unwrap().1
    );
    let mut workers = Vec::new();
    for name in ["first", "second"] {
        let input = RefreshWorkerInput {
            url: rig.url.expose_secret().to_string(),
            endpoint: issuer.endpoint.clone(),
            head: head.clone(),
            barrier: barrier.path().to_path_buf(),
            name: name.into(),
        };
        let mut child = tokio::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", &filter, "--nocapture"])
            .env("BRASSCLAW_CODEX_REFRESH_WORKER", "1")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut pipe = child.stdin.take().unwrap();
        pipe.write_all(&serde_json::to_vec(&input).unwrap())
            .await
            .unwrap();
        drop(pipe);
        workers.push(child);
    }
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        while !barrier.path().join("first").exists() || !barrier.path().join("second").exists() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("both actual child processes acknowledged their native admissions");
    std::fs::write(barrier.path().join("release"), b"release").unwrap();
    for child in workers {
        let output =
            tokio::time::timeout(std::time::Duration::from_secs(20), child.wait_with_output())
                .await
                .unwrap()
                .unwrap();
        assert!(
            output.status.success(),
            "child worker failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "the exact child worker test must actually execute"
        );
    }
    assert_eq!(
        issuer.state.token_calls.load(Ordering::SeqCst),
        2,
        "one login exchange and one shared refresh across two OS processes"
    );
    let rows = rig
        .pool
        .get()
        .await
        .unwrap()
        .query(
            "SELECT data FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1",
            &[&"native-device-test"],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let action: brassclaw_auth::CredentialRefreshAction =
        serde_json::from_value(rows[0].get(0)).unwrap();
    assert!(action.receipt().unwrap().confirmed_refresh());
}
