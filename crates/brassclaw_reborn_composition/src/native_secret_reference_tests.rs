//! Actual PostgreSQL/FS transport tests for the shared secret setup primitives.
use brassclaw_host_api::{InvocationId, ResourceScope, SecretHandle, UserId};
use brassclaw_secrets::{
    FilesystemSecretStore, PgSecretStore, SecretCustodySource, SecretLease, SecretStore,
    SecretsCrypto,
};
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_expiring_secret_cannot_be_reinterpreted_as_nonexpiring_ciphertext() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let namespace = "native-expiry-purpose";
    let native = store(&rig.pool, namespace);
    let owner = scope();
    let handle = SecretHandle::new("finite-ciphertext-purpose").unwrap();
    native
        .put_with_expiry(
            owner.clone(),
            handle.clone(),
            "finite protected token".into(),
            Some(chrono::Utc::now() + chrono::Duration::seconds(60)),
        )
        .await
        .unwrap();
    let original = native.record_reference(&owner, &handle).await.unwrap();
    let client = rig.pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT scope,ciphertext FROM brassclaw_secrets WHERE tenant_id=$1 AND name=$2",
            &[&namespace, &handle.as_str()],
        )
        .await
        .unwrap();
    let key: String = row.get(0);
    let ciphertext: String = row.get(1);
    let stripped = ciphertext.strip_prefix("timed-v1:").unwrap();
    client.execute("UPDATE brassclaw_secrets SET ciphertext=$4 WHERE tenant_id=$1 AND scope=$2 AND name=$3",
        &[&namespace, &key, &handle.as_str(), &stripped]).await.unwrap();
    assert!(native.metadata(&owner, &handle).await.is_err());
    assert!(native.record_reference(&owner, &handle).await.is_err());
    assert!(native.lease_once(&owner, &handle).await.is_err());
    assert!(
        native
            .lease_once_if_record_reference(&owner, &handle, "mismatched-generation")
            .await
            .is_err(),
        "a stale reference must not hide a corrupted encryption purpose"
    );
    assert!(
        native
            .put(
                owner.clone(),
                handle.clone(),
                "must not replace corrupt generation".into()
            )
            .await
            .is_err()
    );
    let actual: String = client
        .query_one(
            "SELECT ciphertext FROM brassclaw_secrets WHERE tenant_id=$1 AND scope=$2 AND name=$3",
            &[&namespace, &key, &handle.as_str()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(actual, stripped);
    client.execute("UPDATE brassclaw_secrets SET ciphertext=$4 WHERE tenant_id=$1 AND scope=$2 AND name=$3",
        &[&namespace, &key, &handle.as_str(), &ciphertext]).await.unwrap();
    assert_eq!(
        native.record_reference(&owner, &handle).await.unwrap(),
        original
    );
    let selected = native.lease_once(&owner, &handle).await.unwrap();
    assert!(selected.secret_expires_at.is_some());
    native.revoke(&owner, selected.id).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_secret_consume_rechecks_issuer_expiry_after_actual_delayed_delete_ack() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let native = store(&rig.pool, "native-delayed-issuer-expiry");
    let owner = scope();
    let handle = SecretHandle::new("finite-before-delayed-commit").unwrap();
    let deadline = chrono::Utc::now() + chrono::Duration::seconds(3);
    native
        .put_with_expiry(
            owner.clone(),
            handle.clone(),
            "finite protected material".into(),
            Some(deadline),
        )
        .await
        .unwrap();
    let reference = native.record_reference(&owner, &handle).await.unwrap();
    let lease = native.lease_once(&owner, &handle).await.unwrap();
    rig.pool.get().await.unwrap().batch_execute(
        "CREATE FUNCTION delay_expiring_lease_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF OLD.scope LIKE 'lease-v2:%' THEN PERFORM pg_sleep(4); END IF; RETURN OLD; END $$; CREATE TRIGGER delay_expiring_lease_delete BEFORE DELETE ON brassclaw_secrets FOR EACH ROW EXECUTE FUNCTION delay_expiring_lease_delete();"
    ).await.unwrap();
    assert!(
        native
            .consume(&owner, lease.id)
            .await
            .unwrap_err()
            .is_expired()
    );
    assert!(
        native
            .leases_for_scope(&owner)
            .await
            .unwrap()
            .iter()
            .all(|remaining| remaining.id != lease.id),
        "the acknowledged consumption must stay consumed after deadline rejection"
    );
    assert!(native.consume(&owner, lease.id).await.is_err());
    assert_eq!(
        native.record_reference(&owner, &handle).await.unwrap(),
        reference
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_historical_encrypted_lease_without_selected_generation_fails_closed() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use brassclaw_secrets::{SecretLeaseId, SecretLeaseStatus, SecretStoreError};
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let namespace = "native-unproven-lease";
    let owner = scope();
    let handle = SecretHandle::new("current-with-historical-lease").unwrap();
    let native = store(&rig.pool, namespace);
    native
        .put(
            owner.clone(),
            handle.clone(),
            "current protected value".into(),
        )
        .await
        .unwrap();
    let reference = native.record_reference(&owner, &handle).await.unwrap();
    let lease_id = SecretLeaseId::new();
    // Genuine old encrypted payload and its historical AAD, not a forged new
    // reference or a test-only bypass of the real stored decoder.
    let payload = serde_json::to_vec(&serde_json::json!({"scope":owner,"handle":handle})).unwrap();
    let crypto = SecretsCrypto::new("native-secret-reference-master-key-2026".into()).unwrap();
    let (encrypted, mut salt) = crypto
        .encrypt(&payload, format!("pg-secret-lease:{lease_id}").as_bytes())
        .unwrap();
    salt.extend_from_slice(&encrypted);
    let client = rig.pool.get().await.unwrap();
    client.execute("INSERT INTO brassclaw_secrets(tenant_id,scope,name,ciphertext,key_version) VALUES($1,$2,'payload',$3,1)", &[&namespace,&format!("lease:{lease_id}"),&STANDARD.encode(salt)]).await.unwrap();
    assert_eq!(
        native.consume(&owner, lease_id).await.unwrap_err(),
        SecretStoreError::LeaseRevoked { lease_id }
    );
    assert_eq!(
        native.record_reference(&owner, &handle).await.unwrap(),
        reference
    );
    assert_eq!(
        native.revoke(&owner, lease_id).await.unwrap().status,
        SecretLeaseStatus::Revoked
    );
    let fresh = native.lease_once(&owner, &handle).await.unwrap();
    use secrecy::ExposeSecret;
    assert_eq!(
        native
            .consume(&owner, fresh.id)
            .await
            .unwrap()
            .expose_secret(),
        "current protected value"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_generation_selected_lease_consumption_does_not_replay_after_lost_commit_reply() {
    use brassclaw_pg::test_support::{AckCutRule, AckCutTransport};
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let namespace = "native-lease-consumption-ack";
    let direct = store(&rig.pool, namespace);
    let owner = scope();
    let handle = SecretHandle::new("selected-before-lost-consumption-reply").unwrap();
    direct
        .put(
            owner.clone(),
            handle.clone(),
            "selected protected value".into(),
        )
        .await
        .unwrap();
    let reference = direct.record_reference(&owner, &handle).await.unwrap();
    let lease = direct.lease_once(&owner, &handle).await.unwrap();
    let transport =
        AckCutTransport::start(rig.url.expose_secret(), vec![AckCutRule::next_commit(1)])
            .await
            .unwrap();
    let through_wire = store(&transport.pool, namespace);
    transport.arm(1).unwrap();
    assert!(through_wire.consume(&owner, lease.id).await.is_err());
    assert_eq!(transport.cut_count(), 1);
    assert!(
        direct.consume(&owner, lease.id).await.is_err(),
        "unknown committed consumption never returns material twice"
    );
    assert_eq!(
        direct.record_reference(&owner, &handle).await.unwrap(),
        reference
    );
    transport.shutdown().await.unwrap();
}

#[path = "../../brassclaw_secrets/tests/support/reference_contract.rs"]
mod contract;

fn scope() -> ResourceScope {
    ResourceScope::local_default(
        UserId::new("secret-reference-owner").unwrap(),
        InvocationId::new(),
    )
    .unwrap()
}
fn store(pool: &Arc<brassclaw_pg::PgPool>, namespace: &str) -> Arc<PgSecretStore> {
    Arc::new(
        PgSecretStore::new(
            pool.as_ref().clone(),
            "native-secret-reference-master-key-2026".into(),
            namespace,
        )
        .unwrap(),
    )
}

async fn insert_legacy_native_secret_fixture(
    pool: &Arc<brassclaw_pg::PgPool>,
    namespace: &str,
    owner: &ResourceScope,
    handle: &SecretHandle,
) -> (String, String) {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use brassclaw_secrets::filesystem_secret_aad;
    let crypto = SecretsCrypto::new("native-secret-reference-master-key-2026".into()).unwrap();
    let (encrypted, salt) = crypto
        .encrypt(b"legacy-material", &filesystem_secret_aad(owner, handle))
        .unwrap();
    // Actual pre-v2 owner-key/envelope shape from pg_store.rs. Defaults
    // contain agent/project IDs; AAD still binds the full tenant/user owner.
    let agent = owner
        .agent_id
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let project = owner
        .project_id
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let legacy_key = format!("secret:{agent}/{project}/{}", handle.as_str());
    let mut encoded = salt;
    encoded.extend_from_slice(&encrypted);
    let ciphertext = STANDARD.encode(encoded);
    let client = pool.get().await.unwrap();
    client.execute("INSERT INTO brassclaw_secrets(tenant_id,scope,name,ciphertext,key_version) VALUES($1,$2,$3,$4,1)", &[&namespace, &legacy_key, &handle.as_str(), &ciphertext]).await.unwrap();
    (legacy_key, ciphertext)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pg_conditional_lease_uses_actual_selected_generation() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    contract::verify_selected_lease_contract(
        store(&rig.pool, "native-selected-lease"),
        store(&rig.pool, "native-selected-lease"),
        scope(),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pg_conditional_lease_rechecks_actual_writer_after_row_lock_wait() {
    use std::time::Duration;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let namespace = "native-selected-lease-row-lock";
    let native = store(&rig.pool, namespace);
    let owner = scope();
    let handle = SecretHandle::new("selected-before-writer-lock").unwrap();
    native
        .put(owner.clone(), handle.clone(), "original before lock".into())
        .await
        .unwrap();
    let expected = native
        .record_reference(&owner, &handle)
        .await
        .unwrap()
        .unwrap();
    let mut writer = rig.pool.get().await.unwrap();
    let transaction = writer.transaction().await.unwrap();
    assert_eq!(
        transaction
            .query(
                "SELECT scope FROM brassclaw_secrets WHERE tenant_id=$1 AND name=$2 FOR UPDATE",
                &[&namespace, &handle.as_str()],
            )
            .await
            .unwrap()
            .len(),
        1
    );
    let pid: i32 = transaction
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    let issuer = {
        let native = native.clone();
        let owner = owner.clone();
        let handle = handle.clone();
        let expected = expected.clone();
        tokio::spawn(async move {
            native
                .lease_once_if_record_reference(&owner, &handle, &expected)
                .await
        })
    };
    // Observe the actual database wait rather than assuming a sleep scheduled
    // the issuer. This watchdog is test scheduling, not a product deadline.
    let observer = rig.pool.get().await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let blocked: bool = observer.query_one(
                "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid)))",
                &[&pid],
            ).await.unwrap().get(0);
            if blocked { break; }
            tokio::task::yield_now().await;
        }
    }).await.expect("conditional issuer must reach the real held row");
    assert!(
        tokio::time::timeout(
            Duration::from_secs(10),
            native.put_if_record_reference_in_transaction(
                &transaction,
                owner.clone(),
                handle.clone(),
                "replacement under writer lock".into(),
                Some(&expected),
            )
        )
        .await
        .expect("writer must not wait on a lease issuer's inverse advisory lock")
        .unwrap()
        .is_some()
    );
    transaction.commit().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(10), issuer)
            .await
            .expect("issuer must finish after the real writer commits")
            .unwrap()
            .unwrap()
            .is_none()
    );
    assert!(
        native.leases_for_scope(&owner).await.unwrap().is_empty(),
        "a mismatched generation creates no lease"
    );
    let current = native
        .record_reference(&owner, &handle)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(current, expected);
    let mut client = rig.pool.get().await.unwrap();
    let transaction = client.transaction().await.unwrap();
    let lease = native
        .lease_once_if_record_reference_in_transaction(&transaction, &owner, &handle, &current)
        .await
        .unwrap()
        .unwrap();
    transaction.rollback().await.unwrap();
    assert!(
        native.consume(&owner, lease.id).await.is_err(),
        "rolled back issuance cannot be consumed"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_persistent_fs_conditional_lease_never_releases_replacement_after_write_wait() {
    use brassclaw_filesystem::PostgresRootFilesystem;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let first = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    first.run_migrations().await.unwrap();
    let second = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    cas_aba::verify_conditional_lease_issuance(
        first,
        second,
        scope(),
        Arc::new(SecretsCrypto::new("native-secret-reference-master-key-2026".into()).unwrap()),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pg_secret_reference_cas_and_readonly_comparison_keep_exact_generations() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let namespace = "native-secret-reference";
    let a = store(&rig.pool, namespace);
    let b = store(&rig.pool, namespace);
    let owner = scope();
    contract::verify_reference_contract(a.clone(), b, owner.clone()).await;
    let handle = SecretHandle::new("generation-reference-probe").unwrap();
    let original = a.record_reference(&owner, &handle).await.unwrap().unwrap();
    let mut client = rig.pool.get().await.unwrap();
    let held = client.transaction().await.unwrap();
    assert_eq!(
        held.query(
            "SELECT scope FROM brassclaw_secrets WHERE tenant_id=$1 AND name=$2 FOR UPDATE",
            &[&namespace, &handle.as_str()]
        )
        .await
        .unwrap()
        .len(),
        1
    );
    // A production write-lock remains held until after these ACTUAL reads.
    // This private watchdog detects an accidental locking/migrating reader;
    // it is neither a product timeout nor a performance estimate.
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        assert_eq!(
            a.record_reference(&owner, &handle).await.unwrap(),
            Some(original.clone())
        );
        let alpha = a
            .matches(&owner, &handle, &"successor-alpha".into())
            .await
            .unwrap();
        let beta = a
            .matches(&owner, &handle, &"successor-beta".into())
            .await
            .unwrap();
        assert_ne!(alpha, beta);
        assert_eq!(
            a.record_reference(&owner, &handle).await.unwrap(),
            Some(original.clone())
        );
    })
    .await
    .expect("authenticated read-only snapshot must not wait on the held row");
    held.rollback().await.unwrap();
    let wrong = PgSecretStore::new(
        rig.pool.as_ref().clone(),
        "wrong-secret-reference-master-key-2026".into(),
        namespace,
    )
    .unwrap();
    assert!(wrong.record_reference(&owner, &handle).await.is_err());
    assert!(
        wrong
            .lease_once_if_record_reference(&owner, &handle, "mismatched-generation")
            .await
            .is_err(),
        "a stale reference must not hide an invalid encryption key"
    );
    assert!(
        wrong
            .matches(&owner, &handle, &"candidate".into())
            .await
            .is_err()
    );
    assert!(
        wrong
            .put_if_record_reference(owner, handle, "replacement".into(), Some(&original))
            .await
            .is_err()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_persistent_fs_secret_reference_cas_uses_independent_real_owners() {
    use brassclaw_filesystem::{PostgresRootFilesystem, ScopedFilesystem};
    use brassclaw_host_api::{MountAlias, MountGrant, MountPermissions, MountView, VirtualPath};
    use brassclaw_secrets::{FilesystemSecretStore, SecretsCrypto};
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let owner = scope();
    let root_a = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    root_a.run_migrations().await.unwrap();
    let root_b = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    let mounts = MountView::new(vec![MountGrant::new(
        MountAlias::new("/secrets").unwrap(),
        VirtualPath::new(format!(
            "/tenants/{}/users/{}/secrets",
            owner.tenant_id, owner.user_id
        ))
        .unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    let crypto =
        Arc::new(SecretsCrypto::new("native-secret-reference-master-key-2026".into()).unwrap());
    let a = Arc::new(FilesystemSecretStore::new(
        Arc::new(ScopedFilesystem::with_fixed_view(root_a, mounts.clone())),
        crypto.clone(),
    ));
    let b = Arc::new(FilesystemSecretStore::new(
        Arc::new(ScopedFilesystem::with_fixed_view(root_b, mounts)),
        crypto,
    ));
    contract::verify_reference_contract(a.clone(), b.clone(), owner.clone()).await;
    contract::verify_selected_lease_contract(a, b, owner).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_secret_reference_write_survives_real_commit_ack_loss_without_rewrite() {
    use brassclaw_pg::test_support::{AckCutRule, AckCutTransport};
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let transport =
        AckCutTransport::start(rig.url.expose_secret(), vec![AckCutRule::next_commit(1)])
            .await
            .unwrap();
    let namespace = "native-secret-reference-ack";
    let proxied = store(&transport.pool, namespace);
    let reader = store(&rig.pool, namespace);
    let owner = scope();
    let handle = SecretHandle::new("original-reference-commit-loss").unwrap();
    transport.arm(1).unwrap();
    assert!(
        proxied
            .put_if_record_reference(
                owner.clone(),
                handle.clone(),
                "retained-original".into(),
                None
            )
            .await
            .is_err()
    );
    assert_eq!(transport.cut_count(), 1);
    let original = reader
        .record_reference(&owner, &handle)
        .await
        .unwrap()
        .unwrap();
    assert!(
        reader
            .matches(&owner, &handle, &"retained-original".into())
            .await
            .unwrap()
    );
    assert!(
        proxied
            .put_if_record_reference(owner.clone(), handle.clone(), "changed-retry".into(), None)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        reader.record_reference(&owner, &handle).await.unwrap(),
        Some(original)
    );
    assert!(
        reader
            .matches(&owner, &handle, &"retained-original".into())
            .await
            .unwrap()
    );
    drop(proxied);
    transport.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_legacy_secret_reference_reads_remain_pure_until_the_exact_cas_write() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let namespace = "native-secret-reference-legacy";
    let owner = scope();
    let handle = SecretHandle::new("legacy-reference-probe").unwrap();
    let (legacy_key, ciphertext) =
        insert_legacy_native_secret_fixture(&rig.pool, namespace, &owner, &handle).await;
    let client = rig.pool.get().await.unwrap();
    let store = store(&rig.pool, namespace);
    let reference = store
        .record_reference(&owner, &handle)
        .await
        .unwrap()
        .unwrap();
    assert!(
        store
            .matches(&owner, &handle, &"legacy-material".into())
            .await
            .unwrap()
    );
    assert_eq!(
        store.record_reference(&owner, &handle).await.unwrap(),
        Some(reference.clone())
    );
    let rows = client
        .query(
            "SELECT scope,ciphertext FROM brassclaw_secrets WHERE tenant_id=$1 AND name=$2",
            &[&namespace, &handle.as_str()],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<_, String>(0), legacy_key);
    assert_eq!(rows[0].get::<_, String>(1), ciphertext);
    assert!(
        store
            .put_if_record_reference(
                owner.clone(),
                handle.clone(),
                "canonical-successor".into(),
                Some(&reference)
            )
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .matches(&owner, &handle, &"canonical-successor".into())
            .await
            .unwrap()
    );
    let rows = client
        .query(
            "SELECT scope FROM brassclaw_secrets WHERE tenant_id=$1 AND name=$2",
            &[&namespace, &handle.as_str()],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].get::<_, String>(0).starts_with("secret-v2:"));
    assert_ne!(
        store.record_reference(&owner, &handle).await.unwrap(),
        Some(reference)
    );
}

#[path = "../../brassclaw_secrets/tests/support/cas_aba.rs"]
pub(super) mod cas_aba;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_persistent_fs_secret_cas_fences_delete_recreate_between_read_and_write() {
    use brassclaw_filesystem::PostgresRootFilesystem;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let first = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    first.run_migrations().await.unwrap();
    let second = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    let crypto =
        Arc::new(SecretsCrypto::new("native-secret-reference-master-key-2026".into()).unwrap());
    cas_aba::verify_secret_aba(first, second, scope(), crypto).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_persistent_fs_lease_consume_fences_restored_revocation_at_reused_version() {
    use brassclaw_filesystem::PostgresRootFilesystem;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let first = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    first.run_migrations().await.unwrap();
    let second = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    let crypto =
        Arc::new(SecretsCrypto::new("native-secret-reference-master-key-2026".into()).unwrap());
    cas_aba::verify_lease_consumption_aba(first, second, scope(), crypto).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pg_conditional_lease_checks_precondition_before_legacy_upgrade() {
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let namespace = "native-selected-legacy-lease";
    let native = store(&rig.pool, namespace);
    let owner = scope();
    let handle = SecretHandle::new("selected-legacy-generation").unwrap();
    let (key, ciphertext) =
        insert_legacy_native_secret_fixture(&rig.pool, namespace, &owner, &handle).await;
    let selected = native
        .record_reference(&owner, &handle)
        .await
        .unwrap()
        .unwrap();
    assert!(
        native
            .lease_once_if_record_reference(&owner, &handle, "wrong-generation")
            .await
            .unwrap()
            .is_none()
    );
    let client = rig.pool.get().await.unwrap();
    let rows = client
        .query(
            "SELECT scope,ciphertext FROM brassclaw_secrets WHERE tenant_id=$1 AND name=$2",
            &[&namespace, &handle.as_str()],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<_, String>(0), key);
    assert_eq!(
        rows[0].get::<_, String>(1),
        ciphertext,
        "mismatch must not migrate legacy bytes"
    );
    assert!(native.leases_for_scope(&owner).await.unwrap().is_empty());
    let lease = native
        .lease_once_if_record_reference(&owner, &handle, &selected)
        .await
        .unwrap()
        .unwrap();
    let canonical = native
        .record_reference(&owner, &handle)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(
        canonical, selected,
        "the owner has migrated only its matched original generation"
    );
    assert!(
        native
            .lease_once_if_record_reference(&owner, &handle, &selected)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        native
            .consume(&owner, lease.id)
            .await
            .unwrap()
            .expose_secret(),
        "legacy-material"
    );
}
