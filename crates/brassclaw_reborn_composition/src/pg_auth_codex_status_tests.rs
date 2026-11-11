//! Actual provider snapshot and native PostgreSQL/secret owners. Setup alone
//! uses the real local issuer; GET must never refresh or dispatch a model.
use super::*;
use brassclaw_product_workflow::{
    NativeAccessLifetime, NativeAccessStatus, NativeAuthCapability, NativeProviderAuthStatus,
    NativeRefreshStatus, NativeRegistrationStatus,
};

fn status(snapshot: &brassclaw_product_workflow::LlmConfigSnapshot) -> &NativeProviderAuthStatus {
    snapshot
        .providers
        .iter()
        .find(|row| row.id == "openai_codex")
        .unwrap()
        .native_auth
        .as_ref()
        .unwrap()
}
async fn store_account(rig: &crate::runtime::test_pg::PgRig, account: &CredentialAccount) {
    rig.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE brassclaw_product_auth_accounts SET data=$3 WHERE tenant_id=$1 AND id=$2",
            &[
                &TENANT,
                &account.id.to_string(),
                &serde_json::to_value(account).unwrap(),
            ],
        )
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_codex_status_reads_current_binding_expiry_and_original_without_effects() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let home = tempfile::tempdir().unwrap();
    let services = factory(&rig, home.path()).await;
    let auth = services.product_auth.as_ref().unwrap();
    let native = auth.native_codex_owner().unwrap();
    let repo = Arc::new(crate::PgProviderRepo::new(
        rig.pool.as_ref().clone(),
        TENANT,
    ));
    let service = crate::RebornLlmConfigService::new(
        services.llm_key_store(),
        rig.pool.clone(),
        repo.clone(),
        TENANT,
    )
    .with_native_product_auth(auth);
    let absent = service.snapshot(caller()).await.unwrap();
    assert_eq!(status(&absent).capability, NativeAuthCapability::Available);
    assert_eq!(
        status(&absent).registration,
        NativeRegistrationStatus::Absent
    );
    assert!(
        absent
            .providers
            .iter()
            .filter(|row| row.adapter != "openai_codex")
            .all(|row| row.native_auth.is_none())
    );
    let issuer = Issuer::start(IssuerMode::NearExpiry).await;
    let (_, head, original) = published_login_on_pool(&rig.pool, &native, &issuer).await;
    let row = repo
        .get_revision("openai_codex", false)
        .await
        .unwrap()
        .unwrap();
    let mut definition = row.definition;
    definition.default_base_url = Some("https://native-device.example/backend-api".into());
    repo.replace_revision(definition, Some(row.revision))
        .await
        .unwrap();
    let calls = issuer.state.calls.load(Ordering::SeqCst);
    let snapshot = service.snapshot(caller()).await.unwrap();
    let observed = status(&snapshot);
    assert_eq!(observed.registration, NativeRegistrationStatus::Bound);
    assert_eq!(observed.access, NativeAccessStatus::Current);
    assert_eq!(observed.lifetime, NativeAccessLifetime::FiniteDeadline);
    assert_eq!(observed.account_expires_at, original.access_expires_at);
    assert_eq!(observed.refresh, NativeRefreshStatus::Present);
    assert!(!original.is_authorized_for_requester(None));
    // A near-expiry GET must not enter normal refresh admission.
    assert!(
        rig.pool
            .get()
            .await
            .unwrap()
            .query(
                "SELECT id FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1",
                &[&TENANT],
            )
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        native
            .read_account(&original.scope, original.id)
            .await
            .unwrap()
            .unwrap()
            .0,
        original
    );
    let wire = serde_json::to_string(observed).unwrap();
    for private in [
        "access-http-original",
        "refresh-http-original",
        "public-native-client",
        "scope",
        "handle",
        "head_reference",
        "published_account_ref",
        "ciphertext",
    ] {
        assert!(
            !wire.contains(private),
            "private field/material escaped: {private}"
        );
    }
    // Account index corruption cannot appear locally current.
    rig.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE brassclaw_product_auth_accounts SET user_id=$3 WHERE tenant_id=$1 AND id=$2",
            &[&TENANT, &original.id.to_string(), &"foreign-index-user"],
        )
        .await
        .unwrap();
    assert_eq!(
        status(&service.snapshot(caller()).await.unwrap()).registration,
        NativeRegistrationStatus::InvalidBinding
    );
    rig.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE brassclaw_product_auth_accounts SET user_id=$3 WHERE tenant_id=$1 AND id=$2",
            &[
                &TENANT,
                &original.id.to_string(),
                &original.scope.resource.user_id.as_str(),
            ],
        )
        .await
        .unwrap();
    // Corrupt JSON cannot be treated as a missing registration or account.
    rig.pool.get().await.unwrap().execute(
        "UPDATE brassclaw_product_auth_accounts SET data='{}'::jsonb WHERE tenant_id=$1 AND id=$2",
        &[&TENANT, &original.id.to_string()],
    ).await.unwrap();
    assert_eq!(
        status(&service.snapshot(caller()).await.unwrap()).registration,
        NativeRegistrationStatus::InvalidBinding
    );
    store_account(&rig, &original).await;
    let mut changed = original.clone();
    changed.status = CredentialAccountStatus::Revoked;
    store_account(&rig, &changed).await;
    let revoked = service.snapshot(caller()).await.unwrap();
    assert_eq!(
        status(&revoked).account.as_ref().unwrap().status,
        CredentialAccountStatus::Revoked
    );
    assert_eq!(status(&revoked).access, NativeAccessStatus::Ineligible);
    changed.status = CredentialAccountStatus::Configured;
    changed.access_expires_at = Some(Utc::now() - chrono::Duration::seconds(1));
    store_account(&rig, &changed).await;
    let expired = service.snapshot(caller()).await.unwrap();
    assert_eq!(status(&expired).access, NativeAccessStatus::Expired);
    assert_eq!(
        status(&expired).recovery,
        Some(brassclaw_auth::CredentialRecoveryReason::AccountExpired)
    );
    store_account(&rig, &original).await;
    // This is a real original admission, deliberately not transported. GET must
    // show it without creating an ID, submitting it or applying a receipt.
    let selected = crate::pg_auth_product_services::codex_refresh::CodexAccessSelection {
        namespace: TENANT,
        provider_id: "openai_codex",
        head: &head,
        api_base_url: "https://native-device.example/backend-api",
        margin_secs: 3600,
    };
    let _admitted = native
        .admit_codex_refresh(&selected)
        .await
        .unwrap()
        .unwrap();
    let blocked = service.snapshot(caller()).await.unwrap();
    let observed = status(&blocked);
    assert_eq!(observed.refresh, NativeRefreshStatus::BlockedOriginal);
    assert_eq!(
        observed.blocking_refresh.as_ref().unwrap().outcome,
        brassclaw_auth::CredentialRefreshOutcomeKind::Unconfirmed
    );
    // Valid terminal legacy originals are retained history, not current success.
    // More than 64 must remain readable without truncation or an invented cap.
    for _ in 0..70 {
        let historical = brassclaw_auth::CredentialRefreshAction {
            original: brassclaw_auth::CredentialRefreshOriginal {
                id: brassclaw_auth::CredentialRefreshId::new(),
                scope: original.scope.clone(),
                provider: original.provider.clone(),
                account_id: original.id,
                requester_extension: None,
            },
            expected_account: original.clone(),
            destinations: None,
            source: None,
            outcome: brassclaw_auth::CredentialRefreshOutcome::NotSubmitted(
                brassclaw_auth::CredentialRefreshStopReason::HostPreparationFailed,
            ),
            publication: brassclaw_auth::CredentialRefreshPublication::Pending,
            published_account: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(!historical.blocks_credential(original.refresh_secret.as_ref().unwrap()));
        let id = historical.original.id.to_string();
        let scope_ref = historical.original.scope_ref().unwrap();
        rig.pool.get().await.unwrap().execute(
            "INSERT INTO brassclaw_product_auth_refreshes (tenant_id,scope_ref,id,account_id,refresh_handle,blocks_refresh,data) VALUES ($1,$2,$3,$4,$5,false,$6)",
            &[&TENANT, &scope_ref, &id, &original.id.to_string(), &original.refresh_secret.as_ref().unwrap().as_str(), &serde_json::to_value(historical).unwrap()],
        ).await.unwrap();
    }
    assert_eq!(
        status(&service.snapshot(caller()).await.unwrap()).blocking_refresh,
        observed.blocking_refresh
    );
    // The old partial index would hide this false flag; complete source reading
    // must detect its disagreement with the blocking domain original.
    rig.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE brassclaw_product_auth_refreshes SET blocks_refresh=false WHERE tenant_id=$1",
            &[&TENANT],
        )
        .await
        .unwrap();
    assert!(service.snapshot(caller()).await.is_err());
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), calls);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_codex_status_unknown_secret_lifetime_and_storage_failure_are_not_ready() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let home = tempfile::tempdir().unwrap();
    let services = factory(&rig, home.path()).await;
    let auth = services.product_auth.as_ref().unwrap();
    let native = auth.native_codex_owner().unwrap();
    let issuer = Issuer::start(IssuerMode::Success).await;
    let (_, _, mut account) = published_login_on_pool(&rig.pool, &native, &issuer).await;
    let repo = Arc::new(crate::PgProviderRepo::new(
        rig.pool.as_ref().clone(),
        TENANT,
    ));
    let row = repo
        .get_revision("openai_codex", false)
        .await
        .unwrap()
        .unwrap();
    let mut definition = row.definition;
    definition.default_base_url = Some("https://native-device.example/backend-api".into());
    repo.replace_revision(definition, Some(row.revision))
        .await
        .unwrap();
    let service = crate::RebornLlmConfigService::new(
        services.llm_key_store(),
        rig.pool.clone(),
        repo,
        TENANT,
    )
    .with_native_product_auth(auth);
    let calls = issuer.state.calls.load(Ordering::SeqCst);
    account.access_expires_at = None;
    store_account(&rig, &account).await;
    let mut connection = rig.pool.get().await.unwrap();
    let tx = connection.transaction().await.unwrap();
    native
        .secret_store
        .put_in_transaction(
            &tx,
            account.scope.resource.clone(),
            account.access_secret.clone().unwrap(),
            "controlled-status-material".into(),
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    drop(connection);
    let unknown = service.snapshot(caller()).await.unwrap();
    assert_eq!(status(&unknown).access, NativeAccessStatus::UnknownLifetime);
    assert_eq!(status(&unknown).lifetime, NativeAccessLifetime::Unknown);
    let mut connection = rig.pool.get().await.unwrap();
    let tx = connection.transaction().await.unwrap();
    native
        .secret_store
        .put_with_expiry_in_transaction(
            &tx,
            account.scope.resource.clone(),
            account.access_secret.clone().unwrap(),
            "controlled-status-material".into(),
            Some(Utc::now() - chrono::Duration::seconds(1)),
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    drop(connection);
    assert_eq!(
        status(&service.snapshot(caller()).await.unwrap()).access,
        NativeAccessStatus::Expired
    );
    account.access_secret = None;
    store_account(&rig, &account).await;
    assert_eq!(
        status(&service.snapshot(caller()).await.unwrap()).access,
        NativeAccessStatus::Absent
    );
    // Genuine storage failure aborts this GET. It cannot become no-account or
    // continue querying/committing an aborted repeatable-read transaction.
    rig.pool
        .get()
        .await
        .unwrap()
        .batch_execute(
            "ALTER TABLE brassclaw_product_auth_accounts RENAME TO status_test_hidden_accounts",
        )
        .await
        .unwrap();
    assert!(service.snapshot(caller()).await.is_err());
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), calls);
}
