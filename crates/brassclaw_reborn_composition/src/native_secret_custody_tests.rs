//! Actual native owners and caller lock ordering; no substituted persistence.
use brassclaw_filesystem::{PostgresRootFilesystem, ScopedFilesystem};
use brassclaw_host_api::{
    InvocationId, MountAlias, MountGrant, MountPermissions, MountView, ResourceScope, SecretHandle,
    UserId, VirtualPath,
};
use brassclaw_secrets::{
    FilesystemSecretStore, PgSecretStore, SecretCustodySource, SecretCustodyState,
    SecretIdentityClosure, SecretStore, SecretStoreError, SecretsCrypto,
};
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_filesystem_index_creation_is_atomic_across_independent_owners() {
    use brassclaw_filesystem::{IndexKey, IndexKind, IndexName, IndexSpec, RootFilesystem};
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let first = PostgresRootFilesystem::new(rig.pool.as_ref().clone());
    let second = PostgresRootFilesystem::new(rig.pool.as_ref().clone());
    first.run_migrations().await.unwrap();
    let client = rig.pool.get().await.unwrap();
    let before: i64 = client
        .query_one(
            "SELECT count(*) FROM pg_index WHERE indrelid='root_filesystem_entries'::regclass",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    for (number, kind) in [IndexKind::Exact, IndexKind::Prefix, IndexKind::Fts]
        .into_iter()
        .enumerate()
    {
        for iteration in 0..4 {
            let path = VirtualPath::new(format!("/secrets/index-concurrent/{number}/{iteration}"))
                .unwrap();
            let spec = IndexSpec {
                name: IndexName::new("first_use").unwrap(),
                keys: vec![IndexKey::new("value").unwrap()],
                kind: kind.clone(),
            };
            // Includes both first insertion and the already committed spec path.
            for _ in 0..2 {
                let (a, b) = tokio::join!(
                    first.ensure_index(&path, &spec),
                    second.ensure_index(&path, &spec)
                );
                a.unwrap();
                b.unwrap();
            }
            let mut incompatible = spec.clone();
            incompatible.keys = vec![IndexKey::new("different").unwrap()];
            assert!(matches!(
                second.ensure_index(&path, &incompatible).await,
                Err(brassclaw_filesystem::FilesystemError::IndexConflict { .. })
            ));
        }
    }
    let after: i64 = client
        .query_one(
            "SELECT count(*) FROM pg_index WHERE indrelid='root_filesystem_entries'::regclass",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        after - before,
        12,
        "one physical index for each distinct declaration"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_filesystem_index_names_preserve_distinct_unsanitized_identities() {
    use brassclaw_filesystem::{IndexKey, IndexKind, IndexName, IndexSpec, RootFilesystem};
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let first = PostgresRootFilesystem::new(rig.pool.as_ref().clone());
    let second = PostgresRootFilesystem::new(rig.pool.as_ref().clone());
    first.run_migrations().await.unwrap();
    let client = rig.pool.get().await.unwrap();
    let before: i64 = client
        .query_one(
            "SELECT count(*) FROM pg_index WHERE indrelid='root_filesystem_entries'::regclass",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    let long = "x".repeat(90);
    for (number, (left, right)) in [
        (
            "/secrets/index/a-b".to_owned(),
            "/secrets/index/a_b".to_owned(),
        ),
        ("/secrets/index/a/b".into(), "/secrets/index/a_b".into()),
        (
            format!("/secrets/index/{long}-a"),
            format!("/secrets/index/{long}_a"),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let a = VirtualPath::new(left).unwrap();
        let b = VirtualPath::new(right).unwrap();
        let sa = IndexSpec {
            name: IndexName::new("identity").unwrap(),
            keys: vec![IndexKey::new("left").unwrap()],
            kind: IndexKind::Exact,
        };
        let sb = IndexSpec {
            keys: vec![IndexKey::new("right").unwrap()],
            ..sa.clone()
        };
        // Use a distinct public name for the second pair's reused right path.
        let name = IndexName::new(format!("identity_{number}")).unwrap();
        let sa = IndexSpec {
            name: name.clone(),
            ..sa
        };
        let sb = IndexSpec { name, ..sb };
        let (a, b) = tokio::join!(first.ensure_index(&a, &sa), second.ensure_index(&b, &sb));
        a.unwrap();
        b.unwrap();
    }
    let after: i64 = client
        .query_one(
            "SELECT count(*) FROM pg_index WHERE indrelid='root_filesystem_entries'::regclass",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        after - before,
        6,
        "punctuation, path separators and long prefixes cannot alias an index"
    );
}

#[tokio::test]
async fn native_filesystem_failed_index_ddl_rolls_back_its_declaration() {
    use brassclaw_filesystem::{IndexKey, IndexKind, IndexName, IndexSpec, RootFilesystem};
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let filesystem = PostgresRootFilesystem::new(rig.pool.as_ref().clone());
    filesystem.run_migrations().await.unwrap();
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE FUNCTION reject_index_test() RETURNS event_trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'deliberate index failure'; END $$; CREATE EVENT TRIGGER reject_index_test ON ddl_command_start WHEN TAG IN ('CREATE INDEX') EXECUTE FUNCTION reject_index_test()").await.unwrap();
    let path = VirtualPath::new("/secrets/index/failed-ddl").unwrap();
    let spec = IndexSpec {
        name: IndexName::new("retry").unwrap(),
        keys: vec![IndexKey::new("value").unwrap()],
        kind: IndexKind::Exact,
    };
    assert!(filesystem.ensure_index(&path, &spec).await.is_err());
    let count: i64 = client
        .query_one(
            "SELECT count(*) FROM root_filesystem_index_specs WHERE prefix=$1",
            &[&path.as_str()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        count, 0,
        "failed physical creation must not publish a declaration"
    );
    client
        .batch_execute("DROP EVENT TRIGGER reject_index_test; DROP FUNCTION reject_index_test()")
        .await
        .unwrap();
    filesystem.ensure_index(&path, &spec).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_identity_closure_reconciles_actual_lost_commit_ack_and_blocks_late_material() {
    use brassclaw_pg::test_support::{AckCutRule, AckCutTransport};
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    for present in [false, true] {
        let transport =
            AckCutTransport::start(rig.url.expose_secret(), vec![AckCutRule::next_commit(1)])
                .await
                .unwrap();
        let namespace = if present {
            "close-original-commit-loss"
        } else {
            "close-absent-commit-loss"
        };
        let writer = native(&transport.pool, namespace);
        let reader = native(&rig.pool, namespace);
        let owner = scope();
        let handle = SecretHandle::new("closed-before-late-writer").unwrap();
        let context = serde_json::json!({"original_flow":"closure-ack-loss"});
        let expected = if present {
            let original = reader
                .put_original(
                    owner.clone(),
                    handle.clone(),
                    "captured original material".into(),
                    context.clone(),
                )
                .await
                .unwrap()
                .unwrap();
            SecretIdentityClosure::Original {
                reference: original.reference,
                context,
            }
        } else {
            SecretIdentityClosure::Absent { context }
        };
        transport.arm(1).unwrap();
        assert!(
            writer
                .close_identity(owner.clone(), handle.clone(), expected.clone())
                .await
                .is_err()
        );
        assert_eq!(transport.cut_count(), 1);
        let actual = reader.custody(&owner, &handle).await.unwrap().unwrap();
        assert!(expected.accepts(&owner, &actual));
        assert_eq!(
            reader
                .close_identity(owner.clone(), handle.clone(), expected)
                .await
                .unwrap(),
            Some(actual.clone())
        );
        assert!(
            reader
                .put_original(
                    owner.clone(),
                    handle.clone(),
                    "late writer material".into(),
                    serde_json::json!({})
                )
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            reader
                .put(owner.clone(), handle.clone(), "ordinary late writer".into())
                .await
                .is_err()
        );
        assert_eq!(reader.custody(&owner, &handle).await.unwrap(), Some(actual));
    }
}
#[path = "../../brassclaw_secrets/tests/support/custody_contract.rs"]
mod contract;
fn scope() -> ResourceScope {
    ResourceScope::local_default(UserId::new("custody-owner").unwrap(), InvocationId::new())
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_closing_genuine_legacy_ordinary_material_migrates_its_actual_physical_identity() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use brassclaw_secrets::filesystem_secret_aad;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let owner = scope();
    let handle = SecretHandle::new("legacy-ordinary-close").unwrap();
    let namespace = "legacy-ordinary-custody-close";
    let crypto = SecretsCrypto::new("actual-custody-native-master-key-2026".into()).unwrap();
    // Actual pre-v2 envelope/key fixture, as in native_secret_reference_tests.
    // It is encrypted for the real historical AAD, never a renamed modern row.
    let (encrypted, mut salt) = crypto
        .encrypt(
            b"actual old ordinary material",
            &filesystem_secret_aad(&owner, &handle),
        )
        .unwrap();
    salt.extend_from_slice(&encrypted);
    let legacy_key = format!(
        "secret:{}/{}/{}",
        owner
            .agent_id
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default(),
        owner
            .project_id
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default(),
        handle.as_str()
    );
    let client = rig.pool.get().await.unwrap();
    client.execute("INSERT INTO brassclaw_secrets(tenant_id,scope,name,ciphertext,key_version) VALUES($1,$2,$3,$4,1)",
        &[&namespace,&legacy_key,&handle.as_str(),&STANDARD.encode(salt)]).await.unwrap();
    let store = native(&rig.pool, namespace);
    let original = store
        .record_reference(&owner, &handle)
        .await
        .unwrap()
        .unwrap();
    let expected = SecretIdentityClosure::Ordinary {
        reference: original.clone(),
        context: serde_json::json!({"cleanup":"legacy-ordinary"}),
    };
    let closed = store
        .close_identity(owner.clone(), handle.clone(), expected.clone())
        .await
        .unwrap()
        .unwrap();
    assert!(expected.accepts(&owner, &closed));
    assert!(
        matches!(&closed.state,SecretCustodyState::RetiredOrdinary {original_reference,..} if original_reference==&original)
    );
    assert_eq!(
        store.custody(&owner, &handle).await.unwrap(),
        Some(closed.clone())
    );
    assert!(store.lease_once(&owner, &handle).await.is_err());
    let rows = client
        .query(
            "SELECT scope FROM brassclaw_secrets WHERE tenant_id=$1 AND name=$2",
            &[&namespace, &handle.as_str()],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_ne!(rows[0].get::<_, String>(0), legacy_key);
}
fn native(pool: &Arc<brassclaw_pg::PgPool>, namespace: &str) -> Arc<PgSecretStore> {
    Arc::new(
        PgSecretStore::new(
            pool.as_ref().clone(),
            "actual-custody-native-master-key-2026".into(),
            namespace,
        )
        .unwrap(),
    )
}
fn filesystem(pool: &Arc<brassclaw_pg::PgPool>, owner: &ResourceScope) -> Arc<dyn SecretStore> {
    // A private physical mount name is distinct from its actual typed System
    // owner. The reserved logical IDs cannot be placed into a VirtualPath.
    let destination = if owner.is_system() {
        "/secrets/custody-contract-system".to_owned()
    } else {
        format!(
            "/tenants/{}/users/{}/secrets",
            owner.tenant_id, owner.user_id
        )
    };
    let mounts = MountView::new(vec![MountGrant::new(
        MountAlias::new("/secrets").unwrap(),
        VirtualPath::new(destination).unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    Arc::new(FilesystemSecretStore::new(
        Arc::new(ScopedFilesystem::with_fixed_view(
            Arc::new(PostgresRootFilesystem::new(pool.as_ref().clone())),
            mounts,
        )),
        Arc::new(SecretsCrypto::new("actual-custody-native-master-key-2026".into()).unwrap()),
    ))
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pg_original_retained_custody_uses_actual_independent_owners() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    contract::verify(
        native(&rig.pool, "native-custody-user"),
        native(&rig.pool, "native-custody-user"),
        scope(),
    )
    .await;
    contract::verify(
        native(&rig.pool, "native-custody-system"),
        native(&rig.pool, "native-custody-system"),
        ResourceScope::system(),
    )
    .await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_persistent_fs_original_retained_custody_uses_actual_independent_owners() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    PostgresRootFilesystem::new(rig.pool.as_ref().clone())
        .run_migrations()
        .await
        .unwrap();
    let owner = scope();
    contract::verify(
        filesystem(&rig.pool, &owner),
        filesystem(&rig.pool, &owner),
        owner,
    )
    .await;
    let owner = ResourceScope::system();
    contract::verify(
        filesystem(&rig.pool, &owner),
        filesystem(&rig.pool, &owner),
        owner,
    )
    .await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pg_provider_style_lease_then_reference_never_inverts_writer_advisory_lock() {
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let store = native(&rig.pool, "native-custody-read-lock-order");
    let owner = ResourceScope::system();
    let handle = SecretHandle::new("provider-snapshot-lock-probe").unwrap();
    store
        .put(owner.clone(), handle.clone(), "first actual key".into())
        .await
        .unwrap();
    let original = store
        .record_reference(&owner, &handle)
        .await
        .unwrap()
        .unwrap();
    let mut reader_client = rig.pool.get().await.unwrap();
    let reader = tokio_postgres::Client::build_transaction(&mut reader_client)
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let lease = store
        .lease_once_in_transaction(&reader, &owner, &handle)
        .await
        .unwrap();
    assert_eq!(
        store
            .consume_in_transaction(&reader, &owner, lease.id)
            .await
            .unwrap()
            .expose_secret(),
        "first actual key"
    );
    let mut writer_client = rig.pool.get().await.unwrap();
    let writer_pid: i32 = writer_client
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    let writer = writer_client.transaction().await.unwrap();
    let writer_call = store.put_in_transaction(
        &writer,
        owner.clone(),
        handle.clone(),
        "second actual key".into(),
    );
    let reader_call = async {
        // Observe the actual waiting native writer rather than guessing a sleep.
        tokio::time::timeout(std::time::Duration::from_secs(10),async {
            loop {
                let observer=rig.pool.get().await.unwrap();
                let held:bool=observer.query_one("SELECT EXISTS (SELECT 1 FROM pg_locks WHERE pid=$1 AND locktype='advisory' AND granted)",&[&writer_pid]).await.unwrap().get(0);
                if held { break; }
                tokio::task::yield_now().await;
            }
        }).await.expect("actual writer must acquire its normal advisory identity lock");
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            assert_eq!(
                store
                    .authenticated_record_reference_in_transaction(&reader, &owner, &handle)
                    .await
                    .unwrap(),
                Some(original.clone())
            );
            assert!(
                store
                    .matches_in_transaction(&reader, &owner, &handle, &"first actual key".into())
                    .await
                    .unwrap()
            );
        })
        .await
        .expect("read owning its lease row must not wait on a writer advisory lock");
        reader.commit().await.unwrap();
    };
    let (write, ()) = tokio::join!(writer_call, reader_call);
    write.unwrap();
    writer.commit().await.unwrap();
    assert_ne!(
        store.record_reference(&owner, &handle).await.unwrap(),
        Some(original)
    );
    assert!(
        store
            .matches(&owner, &handle, &"second actual key".into())
            .await
            .unwrap()
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pg_custody_original_survives_actual_commit_ack_loss_without_replacement() {
    use brassclaw_pg::test_support::{AckCutRule, AckCutTransport};
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let transport =
        AckCutTransport::start(rig.url.expose_secret(), vec![AckCutRule::next_commit(1)])
            .await
            .unwrap();
    let writer = native(&transport.pool, "native-original-commit-loss");
    let reader = native(&rig.pool, "native-original-commit-loss");
    let owner = scope();
    let handle = SecretHandle::new("first-committed-original").unwrap();
    let context = serde_json::json!({"selected_account":"actual-first-choice"});
    transport.arm(1).unwrap();
    assert!(
        writer
            .put_original(
                owner.clone(),
                handle.clone(),
                "first actual input".into(),
                context.clone()
            )
            .await
            .is_err()
    );
    assert_eq!(transport.cut_count(), 1);
    let original = reader.custody(&owner, &handle).await.unwrap().unwrap();
    assert_eq!(
        reader
            .put_original(
                owner.clone(),
                handle.clone(),
                "first actual input".into(),
                context.clone()
            )
            .await
            .unwrap(),
        Some(original.clone())
    );
    assert!(
        reader
            .put_original(
                owner.clone(),
                handle.clone(),
                "changed retry".into(),
                context
            )
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        reader.custody(&owner, &handle).await.unwrap(),
        Some(original)
    );
    drop(writer);
    transport.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_persistent_fs_ordinary_delete_fences_new_immutable_original_after_recreation() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let first = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    first.run_migrations().await.unwrap();
    let second = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    crate::native_secret_reference_tests::cas_aba::verify_secret_delete_aba(
        first,
        second,
        scope(),
        Arc::new(SecretsCrypto::new("actual-custody-native-master-key-2026".into()).unwrap()),
    )
    .await;
}

fn actual_custody_owner(
    pool: &Arc<brassclaw_pg::PgPool>,
    owner: &ResourceScope,
    persistent_fs: bool,
) -> Arc<dyn SecretStore> {
    if persistent_fs {
        filesystem(pool, owner)
    } else {
        native(pool, "native-custody-unknown-output")
    }
}
fn filesystem_secret_write_commit_cut(handle: &str) -> brassclaw_pg::test_support::AckCutRule {
    use brassclaw_pg::test_support::{AckBoundary, AckCutRule};
    // Reads bind only the path; index initialization binds only its declaration.
    // Select the actual encrypted entry write, then withhold its COMMIT result.
    AckCutRule::after_bind(
        1,
        vec![handle.as_bytes().to_vec(), b"encrypted_value".to_vec()],
        vec![],
        AckBoundary::Commit,
    )
}
async fn verify_unknown_output_and_retirement(persistent_fs: bool) {
    use brassclaw_pg::test_support::{AckCutRule, AckCutTransport};
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    if persistent_fs {
        PostgresRootFilesystem::new(rig.pool.as_ref().clone())
            .run_migrations()
            .await
            .unwrap();
    }
    let owner = scope();
    let direct = actual_custody_owner(&rig.pool, &owner, persistent_fs);
    let original_handle = SecretHandle::new("unknown-output-origin").unwrap();
    let output_handle = SecretHandle::new("unknown-retained-output").unwrap();
    let original = direct
        .put_original(
            owner.clone(),
            original_handle,
            "actual original under unknown output".into(),
            serde_json::json!({"original":"persisted first"}),
        )
        .await
        .unwrap()
        .unwrap();
    let source = SecretCustodySource {
        scope: original.scope,
        handle: original.handle,
        reference: original.reference,
    };
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![if persistent_fs {
            filesystem_secret_write_commit_cut("unknown-retained-output")
        } else {
            AckCutRule::next_commit(1)
        }],
    )
    .await
    .unwrap();
    let proxied = actual_custody_owner(&transport.pool, &owner, persistent_fs);
    transport.arm(1).unwrap();
    assert!(
        proxied
            .put_retained_from_original(owner.clone(), output_handle.clone(), source.clone())
            .await
            .is_err()
    );
    assert_eq!(transport.cut_count(), 1);
    let output = direct
        .custody(&owner, &output_handle)
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(&output.state,SecretCustodyState::Retained { source: actual } if actual==&source)
    );
    assert_eq!(
        direct
            .put_retained_from_original(owner.clone(), output_handle.clone(), source.clone())
            .await
            .unwrap(),
        Some(output.clone())
    );
    assert!(
        direct
            .matches(
                &owner,
                &output_handle,
                &"actual original under unknown output".into()
            )
            .await
            .unwrap()
    );
    drop(proxied);
    transport.shutdown().await.unwrap();
    let old_lease = direct.lease_once(&owner, &output_handle).await.unwrap();
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![if persistent_fs {
            filesystem_secret_write_commit_cut("unknown-retained-output")
        } else {
            AckCutRule::next_commit(1)
        }],
    )
    .await
    .unwrap();
    let proxied = actual_custody_owner(&transport.pool, &owner, persistent_fs);
    transport.arm(1).unwrap();
    assert!(proxied.delete(&owner, &output_handle).await.is_err());
    assert_eq!(transport.cut_count(), 1);
    let retired = direct
        .custody(&owner, &output_handle)
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(&retired.state,SecretCustodyState::RetiredRetained { source: actual,retained_reference } if actual==&source && retained_reference==&output.reference)
    );
    assert!(direct.consume(&owner, old_lease.id).await.is_err());
    assert!(!direct.delete(&owner, &output_handle).await.unwrap());
    assert!(
        direct
            .put(
                owner.clone(),
                output_handle.clone(),
                "must not resurrect after unknown retirement".into()
            )
            .await
            .is_err()
    );
    assert_eq!(
        direct.custody(&owner, &output_handle).await.unwrap(),
        Some(retired)
    );
    drop(proxied);
    transport.shutdown().await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pg_retained_creation_and_retirement_survive_actual_commit_ack_loss() {
    verify_unknown_output_and_retirement(false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_persistent_fs_retained_creation_and_retirement_survive_actual_commit_ack_loss() {
    verify_unknown_output_and_retirement(true).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_persistent_fs_original_survives_actual_commit_ack_loss_without_replacement() {
    use brassclaw_pg::test_support::AckCutTransport;
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    PostgresRootFilesystem::new(rig.pool.as_ref().clone())
        .run_migrations()
        .await
        .unwrap();
    let owner = scope();
    let reader = filesystem(&rig.pool, &owner);
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![filesystem_secret_write_commit_cut(
            "first-fs-original-commit-loss",
        )],
    )
    .await
    .unwrap();
    let writer = filesystem(&transport.pool, &owner);
    let handle = SecretHandle::new("first-fs-original-commit-loss").unwrap();
    let context = serde_json::json!({"selection":"actual captured choice"});
    transport.arm(1).unwrap();
    assert!(
        writer
            .put_original(
                owner.clone(),
                handle.clone(),
                "first actual FS input".into(),
                context.clone()
            )
            .await
            .is_err()
    );
    assert_eq!(transport.cut_count(), 1);
    let original = reader.custody(&owner, &handle).await.unwrap().unwrap();
    assert_eq!(
        reader
            .put_original(
                owner.clone(),
                handle.clone(),
                "first actual FS input".into(),
                context.clone()
            )
            .await
            .unwrap(),
        Some(original.clone())
    );
    assert!(
        reader
            .put_original(
                owner.clone(),
                handle.clone(),
                "changed FS retry".into(),
                context
            )
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        reader.custody(&owner, &handle).await.unwrap(),
        Some(original)
    );
    drop(writer);
    transport.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pg_custody_purpose_and_physical_owner_are_authenticated_before_effects() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let namespace = "native-custody-purpose";
    let store = native(&rig.pool, namespace);
    let owner = scope();
    let handle = SecretHandle::new("native-custody-purpose-input").unwrap();
    let context = serde_json::json!({"original":"actual immutable selection"});
    let original = store
        .put_original(
            owner.clone(),
            handle.clone(),
            "sealed original".into(),
            context.clone(),
        )
        .await
        .unwrap()
        .unwrap();
    let mut client = rig.pool.get().await.unwrap();
    let row = client.query_one(
        "SELECT scope,ciphertext,key_version FROM brassclaw_secrets WHERE tenant_id=$1 AND name=$2",
        &[&namespace, &handle.as_str()],
    ).await.unwrap();
    let physical_key: String = row.get(0);
    let ciphertext: String = row.get(1);
    let version: i32 = row.get(2);
    let held = client.transaction().await.unwrap();
    held.query_one(
        "SELECT scope FROM brassclaw_secrets WHERE tenant_id=$1 AND scope=$2 AND name=$3 FOR UPDATE",
        &[&namespace, &physical_key, &handle.as_str()],
    ).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        assert_eq!(
            store.custody(&owner, &handle).await.unwrap(),
            Some(original.clone())
        );
        assert_eq!(
            store.record_reference(&owner, &handle).await.unwrap(),
            Some(original.reference.clone())
        );
    })
    .await
    .expect("original readback must not wait on an actual held writer row");
    held.rollback().await.unwrap();
    let encoded = ciphertext.strip_prefix("custody-v1:").unwrap();
    let mut changed_ciphertext = ciphertext.clone();
    let last = changed_ciphertext.pop().unwrap();
    changed_ciphertext.push(if last == 'A' { 'B' } else { 'A' });
    for corrupt in [
        encoded.to_owned(),
        format!("custody-v255:{encoded}"),
        changed_ciphertext,
    ] {
        client.execute(
            "UPDATE brassclaw_secrets SET ciphertext=$3 WHERE tenant_id=$1 AND scope=$2 AND name=$4",
            &[&namespace, &physical_key, &corrupt, &handle.as_str()],
        ).await.unwrap();
        assert!(store.custody(&owner, &handle).await.is_err());
        assert!(store.record_reference(&owner, &handle).await.is_err());
        assert!(
            store
                .matches(&owner, &handle, &"sealed original".into())
                .await
                .is_err()
        );
        assert!(store.metadata(&owner, &handle).await.is_err());
        assert!(store.lease_once(&owner, &handle).await.is_err());
        assert!(
            store
                .put_original(
                    owner.clone(),
                    handle.clone(),
                    "sealed original".into(),
                    context.clone()
                )
                .await
                .is_err()
        );
        assert!(
            store
                .put(owner.clone(), handle.clone(), "must not overwrite".into())
                .await
                .is_err()
        );
        assert!(
            store
                .put_if_record_reference(
                    owner.clone(),
                    handle.clone(),
                    "must not overwrite".into(),
                    Some(&original.reference)
                )
                .await
                .is_err()
        );
        assert!(store.delete(&owner, &handle).await.is_err());
        let unchanged = client.query_one(
            "SELECT ciphertext,key_version FROM brassclaw_secrets WHERE tenant_id=$1 AND scope=$2 AND name=$3",
            &[&namespace, &physical_key, &handle.as_str()],
        ).await.unwrap();
        assert_eq!(unchanged.get::<_, String>(0), corrupt);
        assert_eq!(unchanged.get::<_, i32>(1), version);
        assert!(store.leases_for_scope(&owner).await.unwrap().is_empty());
        // Explicit restoration of this test-corrupted row, never product recovery.
        client.execute(
            "UPDATE brassclaw_secrets SET ciphertext=$3 WHERE tenant_id=$1 AND scope=$2 AND name=$4",
            &[&namespace, &physical_key, &ciphertext, &handle.as_str()],
        ).await.unwrap();
        assert_eq!(
            store.custody(&owner, &handle).await.unwrap(),
            Some(original.clone())
        );
    }
    let foreign_namespace = "native-custody-copied-physical-owner";
    client.execute(
        "INSERT INTO brassclaw_secrets(tenant_id,scope,name,ciphertext,key_version) VALUES($1,$2,$3,$4,$5)",
        &[&foreign_namespace, &physical_key, &handle.as_str(), &ciphertext, &version],
    ).await.unwrap();
    let copied = native(&rig.pool, foreign_namespace);
    assert!(copied.custody(&owner, &handle).await.is_err());
    assert!(copied.delete(&owner, &handle).await.is_err());
    let wrong = PgSecretStore::new(
        rig.pool.as_ref().clone(),
        "wrong-native-custody-master-key-2026".into(),
        namespace,
    )
    .unwrap();
    assert!(wrong.custody(&owner, &handle).await.is_err());
    assert!(wrong.delete(&owner, &handle).await.is_err());
    let readonly = client
        .build_transaction()
        .read_only(true)
        .start()
        .await
        .unwrap();
    assert_eq!(
        store
            .custody_in_transaction(&readonly, &owner, &handle)
            .await
            .unwrap(),
        Some(original.clone())
    );
    assert_eq!(
        store
            .inspect_record_reference_in_transaction(&readonly, &owner, &handle)
            .await
            .unwrap(),
        Some(original.reference.clone())
    );
    readonly.commit().await.unwrap();
    assert_eq!(
        store.custody(&owner, &handle).await.unwrap(),
        Some(original)
    );
}
