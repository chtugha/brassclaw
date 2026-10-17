use super::*;

#[cfg(feature = "migrate-from-libsql")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_legacy_provider_import_is_atomic_and_preserves_sources() {
    use sha2::{Digest, Sha256};
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let directory = tempfile::tempdir().unwrap();
    let home = brassclaw_reborn_config::RebornHome::resolve_from_env_parts(
        Some(directory.path().as_os_str().to_os_string()),
        None,
        None,
    )
    .unwrap();
    let source = home.path().join("providers.json");
    let definition = |id: &str, description: &str| {
        serde_json::json!({
            "id":id, "aliases":[], "protocol":"open_ai_completions",
            "default_base_url":"http://127.0.0.1:8000/v1", "base_url_required":true,
            "api_key_required":false, "model_env":"LEGACY_NATIVE_MODEL",
            "default_model":"native-model", "description":description
        })
    };
    let values = vec![
        definition("first-legacy-provider", "First retained legacy provider"),
        definition("second-legacy-provider", "Second retained legacy provider"),
    ];
    let bytes = serde_json::to_vec_pretty(&values).unwrap();
    std::fs::write(&source, &bytes).unwrap();
    let run = |dry| crate::migration::run_migration(&rig.pool, &home, "native-legacy-owner", dry);
    assert!(run(true).await.unwrap().providers_migrated);
    let repo = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), "native-legacy-owner");
    assert!(repo.load_all().await.unwrap().is_empty());
    assert_eq!(std::fs::read(&source).unwrap(), bytes);
    assert_eq!(
        std::fs::read_dir(home.path()).unwrap().count(),
        1,
        "dry-run does not create backups"
    );
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE FUNCTION reject_second_legacy_provider() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.tenant_id='native-legacy-owner' AND NEW.provider_id='second-legacy-provider' THEN RAISE EXCEPTION 'deliberate multi-provider rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_second_legacy_provider BEFORE INSERT ON brassclaw_llm_provider_revisions FOR EACH ROW EXECUTE FUNCTION reject_second_legacy_provider();").await.unwrap();
    assert!(run(false).await.is_err());
    assert!(
        repo.load_all().await.unwrap().is_empty(),
        "failed second record rolls back the first too"
    );
    assert_eq!(std::fs::read(&source).unwrap(), bytes);
    let backup = home.path().join(format!(
        "providers.json.migrated.{}",
        hex::encode(Sha256::digest(&bytes))
    ));
    assert_eq!(std::fs::read(&backup).unwrap(), bytes);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    client.batch_execute("DROP TRIGGER reject_second_legacy_provider ON brassclaw_llm_provider_revisions; DROP FUNCTION reject_second_legacy_provider();").await.unwrap();
    assert!(run(false).await.unwrap().providers_migrated);
    assert_eq!(repo.load_all().await.unwrap().len(), 2);
    assert_eq!(
        std::fs::read(&source).unwrap(),
        bytes,
        "source remains an upgrade artifact"
    );
    assert!(repo.delete("first-legacy-provider").await.unwrap());
    let before = repo
        .get_revision("first-legacy-provider", true)
        .await
        .unwrap()
        .unwrap();
    assert!(run(false).await.unwrap().providers_migrated);
    let after = repo
        .get_revision("first-legacy-provider", true)
        .await
        .unwrap()
        .unwrap();
    assert!(after.inactive);
    assert_eq!(after.revision, before.revision);
    assert_eq!(repo.load_all().await.unwrap().len(), 1);
    let conflicting = serde_json::to_vec(&vec![
        definition("first-legacy-provider", "Conflicting edit"),
        definition("not-imported-provider", "Must remain absent"),
    ])
    .unwrap();
    std::fs::write(&source, &conflicting).unwrap();
    assert!(run(false).await.is_err());
    assert!(repo.get("not-imported-provider").await.unwrap().is_none());
    assert_eq!(std::fs::read(&source).unwrap(), conflicting);
    assert_eq!(std::fs::read(&backup).unwrap(), bytes);
    let mut alias = definition("alias-conflict-provider", "Alias conflict");
    alias["aliases"] = serde_json::json!(["second-legacy-provider"]);
    let conflicting_alias = serde_json::to_vec(&vec![alias]).unwrap();
    std::fs::write(&source, &conflicting_alias).unwrap();
    assert!(run(false).await.is_err());
    assert!(repo.get("alias-conflict-provider").await.unwrap().is_none());
    let new_bytes = serde_json::to_vec(&vec![definition(
        "new-legacy-provider",
        "New source version",
    )])
    .unwrap();
    let new_backup = home.path().join(format!(
        "providers.json.migrated.{}",
        hex::encode(Sha256::digest(&new_bytes))
    ));
    std::fs::write(&source, &new_bytes).unwrap();
    std::fs::write(&new_backup, b"conflicting backup artifact").unwrap();
    assert!(run(false).await.is_err());
    assert!(repo.get("new-legacy-provider").await.unwrap().is_none());
    assert_eq!(
        std::fs::read(&new_backup).unwrap(),
        b"conflicting backup artifact"
    );
    assert_eq!(std::fs::read(&source).unwrap(), new_bytes);
    for invalid in [
        br#"[{"id":"unsupported","api_key":"must-not-import-plaintext"}]"#.as_slice(),
        b"not JSON".as_slice(),
    ] {
        std::fs::write(&source, invalid).unwrap();
        assert!(run(false).await.is_err());
        assert_eq!(std::fs::read(&source).unwrap(), invalid);
    }
}

#[tokio::test]
async fn native_pg_provider_secret_leases_preserve_scope_and_one_shot_consumption() {
    use brassclaw_host_api::{InvocationId, ResourceScope, SecretHandle};
    use brassclaw_secrets::{SecretStore, SecretStoreError};
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let store = brassclaw_secrets::PgSecretStore::new(
        rig.pool.as_ref().clone(),
        secrecy::SecretString::from("native-secret-lease-test-master-key"),
        "default",
    )
    .unwrap();
    let scope = ResourceScope::system();
    let handle = SecretHandle::new("native-provider-key").unwrap();
    store
        .put(
            scope.clone(),
            handle.clone(),
            secrecy::SecretString::from("native-test-material"),
        )
        .await
        .unwrap();
    let lease = store.lease_once(&scope, &handle).await.unwrap();
    let wrong = ResourceScope {
        invocation_id: InvocationId::new(),
        ..scope.clone()
    };
    assert!(matches!(
        store.consume(&wrong, lease.id).await,
        Err(SecretStoreError::UnknownLease { .. })
    ));
    assert!(matches!(
        store.revoke(&wrong, lease.id).await,
        Err(SecretStoreError::UnknownLease { .. })
    ));
    assert!(store.leases_for_scope(&wrong).await.unwrap().is_empty());
    assert_eq!(store.leases_for_scope(&scope).await.unwrap().len(), 1);
    let (one, two) = tokio::join!(
        store.consume(&scope, lease.id),
        store.consume(&scope, lease.id)
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert!(matches!(
        one.as_ref().err().or(two.as_ref().err()),
        Some(SecretStoreError::UnknownLease { .. })
    ));
    assert!(store.leases_for_scope(&scope).await.unwrap().is_empty());
    let second = store.lease_once(&scope, &handle).await.unwrap();
    assert_eq!(
        store.revoke(&scope, second.id).await.unwrap().handle,
        handle
    );
    assert!(matches!(
        store.consume(&scope, second.id).await,
        Err(SecretStoreError::UnknownLease { .. })
    ));
    let third = store.lease_once(&scope, &handle).await.unwrap();
    assert!(store.consume(&scope, third.id).await.is_ok());
    assert!(store.metadata(&scope, &handle).await.unwrap().is_some());
    // The private encrypted storage reader must not loosen public ID parsing.
    assert!(serde_json::from_value::<ResourceScope>(serde_json::to_value(scope).unwrap()).is_err());
}

#[tokio::test]
async fn native_pg_secret_owner_keys_and_authenticated_legacy_upgrade() {
    use base64::Engine as _;
    use brassclaw_host_api::{InvocationId, ResourceScope, SecretHandle, TenantId, UserId};
    use brassclaw_secrets::{SecretStore, SecretsCrypto, filesystem_secret_aad};
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let master = "native-owner-isolation-master-key";
    let store = brassclaw_secrets::PgSecretStore::new(
        rig.pool.as_ref().clone(),
        secrecy::SecretString::from(master),
        "physical-owner-a",
    )
    .unwrap();
    let copied = brassclaw_secrets::PgSecretStore::new(
        rig.pool.as_ref().clone(),
        secrecy::SecretString::from(master),
        "physical-owner-b",
    )
    .unwrap();
    let owner = ResourceScope {
        tenant_id: TenantId::new("logical-tenant-a").unwrap(),
        user_id: UserId::new("logical-user-a").unwrap(),
        ..ResourceScope::system()
    };
    let other_user = ResourceScope {
        user_id: UserId::new("logical-user-b").unwrap(),
        ..owner.clone()
    };
    let other_tenant = ResourceScope {
        tenant_id: TenantId::new("logical-tenant-b").unwrap(),
        ..owner.clone()
    };
    let other_invocation = ResourceScope {
        invocation_id: InvocationId::new(),
        ..owner.clone()
    };
    let handle = SecretHandle::new("same-handle").unwrap();
    for (scope, value) in [
        (&owner, "owner-a-value"),
        (&other_user, "owner-b-value"),
        (&other_tenant, "tenant-b-value"),
    ] {
        store
            .put(
                scope.clone(),
                handle.clone(),
                secrecy::SecretString::from(value),
            )
            .await
            .unwrap();
        let lease = store.lease_once(scope, &handle).await.unwrap();
        assert_eq!(
            store
                .consume(scope, lease.id)
                .await
                .unwrap()
                .expose_secret(),
            value
        );
    }
    // Material follows its owner across invocations; leases do not.
    let lease = store.lease_once(&other_invocation, &handle).await.unwrap();
    assert_eq!(
        store
            .consume(&other_invocation, lease.id)
            .await
            .unwrap()
            .expose_secret(),
        "owner-a-value"
    );
    assert!(store.delete(&other_user, &handle).await.unwrap());
    assert!(
        store
            .metadata(&other_user, &handle)
            .await
            .unwrap()
            .is_none()
    );
    assert!(store.metadata(&owner, &handle).await.unwrap().is_some());
    assert!(
        store
            .metadata(&other_tenant, &handle)
            .await
            .unwrap()
            .is_some()
    );
    let lease = store.lease_once(&owner, &handle).await.unwrap();
    let client = rig.pool.get().await.unwrap();
    // Copy both envelopes verbatim across physical stores using the same key.
    // The new AAD must reject the copy, even for identical logical owners.
    client.execute("INSERT INTO brassclaw_secrets (tenant_id, scope, name, ciphertext, key_version) SELECT 'physical-owner-b', scope, name, ciphertext, key_version FROM brassclaw_secrets WHERE tenant_id = 'physical-owner-a'", &[]).await.unwrap();
    assert!(copied.metadata(&owner, &handle).await.is_err());
    assert!(copied.delete(&owner, &handle).await.is_err());
    assert!(copied.consume(&owner, lease.id).await.is_err());
    assert!(copied.revoke(&owner, lease.id).await.is_err());
    assert!(copied.leases_for_scope(&owner).await.is_err());
    assert_eq!(
        store
            .consume(&owner, lease.id)
            .await
            .unwrap()
            .expose_secret(),
        "owner-a-value"
    );
    let wrong_key = brassclaw_secrets::PgSecretStore::new(
        rig.pool.as_ref().clone(),
        secrecy::SecretString::from("different-owner-isolation-master-key"),
        "physical-owner-a",
    )
    .unwrap();
    assert!(
        wrong_key
            .put(
                owner.clone(),
                handle.clone(),
                secrecy::SecretString::from("replacement")
            )
            .await
            .is_err()
    );
    let lease = store.lease_once(&owner, &handle).await.unwrap();
    assert_eq!(
        store
            .consume(&owner, lease.id)
            .await
            .unwrap()
            .expose_secret(),
        "owner-a-value"
    );

    let legacy_handle = SecretHandle::new("legacy-owner-key").unwrap();
    let crypto = SecretsCrypto::new(secrecy::SecretString::from(master)).unwrap();
    let (ciphertext, salt) = crypto
        .encrypt(
            b"retained-legacy-value",
            &filesystem_secret_aad(&owner, &legacy_handle),
        )
        .unwrap();
    let encoded = base64::engine::general_purpose::STANDARD
        .encode([salt.as_slice(), ciphertext.as_slice()].concat());
    client.execute("INSERT INTO brassclaw_secrets (tenant_id, scope, name, ciphertext, key_version) VALUES ('physical-owner-a', 'secret://legacy-owner-key', 'legacy-owner-key', $1, 1)", &[&encoded]).await.unwrap();
    // Legacy row names did not identify the user. Wrong-owner access may not
    // delete it or upgrade it under a new owner; the error remains explicit.
    assert!(store.metadata(&other_user, &legacy_handle).await.is_err());
    assert!(store.delete(&other_user, &legacy_handle).await.is_err());
    assert!(
        store
            .put(
                other_user.clone(),
                legacy_handle.clone(),
                secrecy::SecretString::from("wrong-owner-replacement")
            )
            .await
            .is_err()
    );
    assert_eq!(client.query_one("SELECT ciphertext FROM brassclaw_secrets WHERE tenant_id = 'physical-owner-a' AND scope = 'secret://legacy-owner-key'", &[]).await.unwrap().get::<_, String>(0), encoded);
    client.batch_execute("CREATE FUNCTION reject_legacy_owner_upgrade() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF OLD.scope = 'secret://legacy-owner-key' THEN RAISE EXCEPTION 'deliberate legacy upgrade rejection'; END IF; RETURN OLD; END $$; CREATE TRIGGER reject_legacy_owner_upgrade BEFORE DELETE ON brassclaw_secrets FOR EACH ROW EXECUTE FUNCTION reject_legacy_owner_upgrade();").await.unwrap();
    let rejected = store.metadata(&owner, &legacy_handle).await;
    client.batch_execute("DROP TRIGGER reject_legacy_owner_upgrade ON brassclaw_secrets; DROP FUNCTION reject_legacy_owner_upgrade();").await.unwrap();
    assert!(rejected.is_err());
    assert_eq!(client.query_one("SELECT count(*) FROM brassclaw_secrets WHERE tenant_id = 'physical-owner-a' AND name = 'legacy-owner-key'", &[]).await.unwrap().get::<_, i64>(0), 1);
    assert!(
        store
            .metadata(&owner, &legacy_handle)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(client.query_one("SELECT count(*) FROM brassclaw_secrets WHERE tenant_id = 'physical-owner-a' AND scope = 'secret://legacy-owner-key'", &[]).await.unwrap().get::<_, i64>(0), 0);
    let legacy_lease = brassclaw_secrets::SecretLeaseId::new();
    let payload =
        serde_json::to_vec(&serde_json::json!({"handle":legacy_handle,"scope":owner})).unwrap();
    let (ciphertext, salt) = crypto
        .encrypt(
            &payload,
            format!("pg-secret-lease:{legacy_lease}").as_bytes(),
        )
        .unwrap();
    let encoded = base64::engine::general_purpose::STANDARD
        .encode([salt.as_slice(), ciphertext.as_slice()].concat());
    client.execute("INSERT INTO brassclaw_secrets (tenant_id, scope, name, ciphertext, key_version) VALUES ('physical-owner-a', $1, 'payload', $2, 1)", &[&format!("lease:{legacy_lease}"), &encoded]).await.unwrap();
    assert_eq!(store.leases_for_scope(&owner).await.unwrap().len(), 1);
    assert_eq!(
        store
            .consume(&owner, legacy_lease)
            .await
            .unwrap()
            .expose_secret(),
        "retained-legacy-value"
    );
    assert!(store.leases_for_scope(&owner).await.unwrap().is_empty());
    // Removal cannot resurrect the legacy credential: the old row was moved.
    assert!(store.delete(&owner, &legacy_handle).await.unwrap());
    assert!(
        store
            .metadata(&owner, &legacy_handle)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn native_database_provider_retains_selected_prompt_cache_policy() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let tenant = "native-cache-policy";
    let repo = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), tenant);
    let mut definition: brassclaw_llm::ProviderDefinition = serde_json::from_value(serde_json::json!({
        "id":"selected-anthropic", "aliases":[], "protocol":"anthropic",
        "default_base_url":"http://127.0.0.1:8000", "base_url_env":null, "base_url_required":true,
        "api_key_env":null, "api_key_required":true, "model_env":"UNUSED_NATIVE_CACHE_MODEL", "default_model":"catalogue-default-model",
        "description":"Native selected cache policy fixture", "extra_headers_env":null, "unsupported_params":[], "setup":null,
        "token_budget":null, "context_window_tokens":131072, "cache_retention":"long"
    })).unwrap();
    repo.upsert(definition.clone()).await.unwrap();
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            secrecy::SecretString::from("native-selected-cache-policy-master-key"),
            "native-cache-key-owner",
        )
        .unwrap(),
    ));
    keys.put(
        "selected-anthropic",
        secrecy::SecretString::from("native-selected-cache-credential"),
    )
    .await
    .unwrap();
    crate::db_config::save_config_changes(
        &rig.pool,
        tenant,
        &[
            ("llm.default.provider_id", Some("selected-anthropic")),
            ("llm.default.model", Some("selected-database-model")),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    for retention in ["long", "short", "none"] {
        definition.cache_retention = Some(retention.into());
        repo.upsert(definition.clone()).await.unwrap();
        let (selected, retained_definition) =
            crate::llm_catalog::resolve_provider_selection_db(&rig.pool, tenant, &keys)
                .await
                .unwrap()
                .unwrap();
        let provider = selected.provider.unwrap();
        assert_eq!(
            provider.cache_retention,
            retention.parse::<brassclaw_llm::CacheRetention>().unwrap()
        );
        assert_eq!(provider.model, "selected-database-model");
        assert_eq!(
            retained_definition.cache_retention.as_deref(),
            Some(retention)
        );
    }
    definition.cache_retention = Some("invalid-database-policy".into());
    repo.upsert(definition).await.unwrap();
    assert!(
        crate::llm_catalog::resolve_provider_selection_db(&rig.pool, tenant, &keys)
            .await
            .is_err()
    );
}

/// Actual OpenAI protocol requests verify cache behavior; this endpoint is
/// controlled test transport, not evidence of deployment model quality.
/// Ordinary chat/No-Match calls use real provider HTTP transport through the
/// global Monty service. The controlled endpoint proves selection/retention,
/// not inference quality or the unfinished model/design/pricing cutover.
#[tokio::test]
async fn native_provider_revision_history_fences_stale_reactivation() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let repo = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), "native-provider-revisions");
    let definition: brassclaw_llm::ProviderDefinition = serde_json::from_value(serde_json::json!({"id":"retained-provider","aliases":[],"protocol":"open_ai_completions",
        "default_base_url":"http://127.0.0.1:8000/v1","base_url_env":null,"base_url_required":true,"api_key_env":null,"api_key_required":false,
        "model_env":"UNUSED_NATIVE_REVISION_MODEL","default_model":"original-model","description":"Original configuration","extra_headers_env":null,
        "unsupported_params":[],"setup":null,"token_budget":null,"context_window_tokens":131072,"cache_retention":null})).unwrap();
    let created = repo
        .replace_revision(definition.clone(), None)
        .await
        .unwrap();
    assert_eq!(created.revision, 1);
    assert!(!created.inactive && !created.is_builtin);
    assert!(matches!(
        repo.replace_revision(definition.clone(), None).await,
        Err(crate::pg_provider_repo::PgProviderRepoError::RevisionConflict)
    ));
    let mut first = definition.clone();
    first.default_model = "first-model".into();
    let mut second = definition.clone();
    second.default_model = "second-model".into();
    let (a, b) = tokio::join!(
        repo.replace_revision(first, Some(1)),
        repo.replace_revision(second, Some(1))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let current = repo
        .get_revision("retained-provider", false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.revision, 2);
    crate::db_config::save_config_changes(
        &rig.pool,
        "native-provider-revisions",
        &[
            ("llm.default.provider_id", Some("retained-provider")),
            ("llm.default.model", Some("selected-model")),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    assert!(repo.delete("retained-provider").await.unwrap());
    assert!(
        repo.get_revision("retained-provider", false)
            .await
            .unwrap()
            .is_none()
    );
    let hidden = repo
        .get_revision("retained-provider", true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(hidden.revision, 3);
    assert!(hidden.inactive);
    assert!(matches!(
        repo.replace_revision(hidden.definition.clone(), Some(2))
            .await,
        Err(crate::pg_provider_repo::PgProviderRepoError::RevisionConflict)
    ));
    let restored = repo
        .replace_revision(hidden.definition.clone(), Some(3))
        .await
        .unwrap();
    assert_eq!(restored.revision, 4);
    assert!(!restored.inactive);
    assert_eq!(
        restored.definition.default_model,
        current.definition.default_model
    );
    let keys = crate::db_config::list_config_keys(&rig.pool, "native-provider-revisions")
        .await
        .unwrap();
    assert!(
        !keys.iter().any(|(key, _)| key.starts_with("llm.default.")),
        "reactivation does not restore a past selection"
    );
    let mut client = rig.pool.get().await.unwrap();
    let original:serde_json::Value=client.query_one("SELECT definition FROM brassclaw_llm_provider_revisions WHERE tenant_id='native-provider-revisions' AND provider_id='retained-provider' AND revision=1",&[]).await.unwrap().get(0);
    assert_eq!(original, serde_json::to_value(&definition).unwrap());
    for sql in [
        "UPDATE brassclaw_llm_provider_revisions SET definition='{}' WHERE tenant_id='native-provider-revisions'",
        "DELETE FROM brassclaw_llm_provider_revisions WHERE tenant_id='native-provider-revisions'",
        "TRUNCATE brassclaw_llm_provider_revisions",
    ] {
        let error = client.batch_execute(sql).await.unwrap_err();
        assert_eq!(error.code().map(|code| code.code()), Some("42501"));
    }
    let error=client.execute("UPDATE brassclaw_llm_providers SET revision=1 WHERE tenant_id='native-provider-revisions' AND id='retained-provider'",&[]).await.unwrap_err();
    assert_eq!(error.code().map(|code| code.code()), Some("23514"));
    let error=client.execute("UPDATE brassclaw_llm_providers SET id='renamed-provider' WHERE tenant_id='native-provider-revisions' AND id='retained-provider'",&[]).await.unwrap_err();
    assert_eq!(error.code().map(|code| code.code()), Some("23514"));
    client.batch_execute("CREATE FUNCTION reject_native_provider_history() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.tenant_id='native-provider-revisions' THEN RAISE EXCEPTION 'deliberate history rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_native_provider_history BEFORE INSERT ON brassclaw_llm_provider_revisions FOR EACH ROW EXECUTE FUNCTION reject_native_provider_history();").await.unwrap();
    let rejected = repo.replace_revision(definition.clone(), Some(4)).await;
    client.batch_execute("DROP TRIGGER reject_native_provider_history ON brassclaw_llm_provider_revisions; DROP FUNCTION reject_native_provider_history();").await.unwrap();
    assert!(rejected.is_err());
    let unchanged = repo
        .get_revision("retained-provider", false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.revision, 4);
    assert_eq!(
        unchanged.definition.default_model,
        restored.definition.default_model
    );
    // Exercise the exact upgrade SQL on a pre-revision table in an isolated
    // transactional schema, preserving operator-owned JSON and tombstones.
    let upgrade = client.transaction().await.unwrap();
    upgrade.batch_execute("CREATE SCHEMA native_provider_upgrade; SET LOCAL search_path TO native_provider_upgrade,pg_catalog; CREATE TABLE brassclaw_llm_providers (tenant_id text NOT NULL,id text NOT NULL,definition jsonb NOT NULL,is_builtin boolean NOT NULL,deleted_at timestamptz,PRIMARY KEY(tenant_id,id));").await.unwrap();
    upgrade.execute("INSERT INTO brassclaw_llm_providers (tenant_id,id,definition,is_builtin,deleted_at) VALUES ('retained-owner','retained-provider',$1,true,now())",&[&original]).await.unwrap();
    upgrade
        .batch_execute(include_str!(
            "../../../../brassclaw_pg/migrations/V138__provider_definition_revisions.sql"
        ))
        .await
        .unwrap();
    let migrated=upgrade.query_one("SELECT p.definition,p.revision,p.is_builtin,p.deleted_at IS NOT NULL,r.definition FROM brassclaw_llm_providers p JOIN brassclaw_llm_provider_revisions r ON r.tenant_id=p.tenant_id AND r.provider_id=p.id AND r.revision=p.revision",&[]).await.unwrap();
    assert_eq!(migrated.get::<_, serde_json::Value>(0), original);
    assert_eq!(migrated.get::<_, i64>(1), 1);
    assert!(migrated.get::<_, bool>(2));
    assert!(migrated.get::<_, bool>(3));
    assert_eq!(migrated.get::<_, serde_json::Value>(4), original);
    upgrade.rollback().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_provider_settings_view_is_coherent_and_resolves_aliases() {
    use brassclaw_product_workflow::LlmConfigService;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let tenant = "native-provider-view";
    let repo = Arc::new(crate::PgProviderRepo::new(
        rig.pool.as_ref().clone(),
        tenant,
    ));
    let definition = serde_json::from_value(serde_json::json!({"id":"canonical-provider","aliases":["legacy-provider"],"protocol":"open_ai_completions",
        "default_base_url":"http://127.0.0.1:8000/v1","base_url_env":null,"base_url_required":true,"api_key_env":"UNUSED_NATIVE_VIEW_KEY","api_key_required":true,
        "model_env":"UNUSED_NATIVE_VIEW_MODEL","default_model":"default-model","description":"generation0","extra_headers_env":null,
        "unsupported_params":[],"setup":null,"token_budget":null,"context_window_tokens":131072,"cache_retention":null})).unwrap();
    repo.upsert(definition).await.unwrap();
    crate::db_config::save_config_changes(
        &rig.pool,
        tenant,
        &[
            ("llm.default.provider_id", Some("LEGACY-PROVIDER")),
            ("llm.default.model", Some("generation0")),
            ("llm.sempai.provider_id", Some("legacy-provider")),
            ("llm.sempai.model", Some("generation0")),
            ("embedding.provider_id", Some("legacy-provider")),
            ("embedding.model", Some("generation0")),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            secrecy::SecretString::from("native-provider-view-master-key-material"),
            "native-view-key-owner",
        )
        .unwrap(),
    ));
    let service = Arc::new(crate::RebornLlmConfigService::new(
        keys,
        rig.pool.clone(),
        repo,
        tenant,
    ));
    let caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        brassclaw_host_api::TenantId::new(tenant).unwrap(),
        brassclaw_host_api::UserId::new("native-view-owner").unwrap(),
        None,
        None,
    );
    rig.pool.resize(1);
    let single_connection =
        tokio::time::timeout(Duration::from_secs(5), service.snapshot(caller.clone())).await;
    rig.pool.resize(16);
    let snapshot = single_connection
        .expect("settings view releases the connection before secret checks")
        .unwrap();
    assert_eq!(snapshot.providers.len(), 1);
    assert!(
        snapshot.providers[0].accepts_api_key,
        "required stored keys are editable without an environment reference"
    );
    assert!(!snapshot.providers[0].api_key_set);
    assert!(
        snapshot.providers[0].is_kohai
            && snapshot.providers[0].is_sempai
            && snapshot.providers[0].is_embedding
    );
    for selected in [
        &snapshot.kohai_active,
        &snapshot.sempai_active,
        &snapshot.embedding_active,
    ] {
        assert_eq!(selected.as_ref().unwrap().provider_id, "canonical-provider");
    }
    let writer_pool = rig.pool.clone();
    let writer = tokio::spawn(async move {
        let mut client = writer_pool.get().await.unwrap();
        for generation in 1..=64 {
            let generation = format!("generation{generation}");
            let transaction = client.transaction().await.unwrap();
            transaction.execute("UPDATE brassclaw_llm_providers SET definition=jsonb_set(definition,'{description}',to_jsonb($1::text)) WHERE tenant_id='native-provider-view' AND id='canonical-provider'",&[&generation]).await.unwrap();
            transaction.execute("UPDATE brassclaw_config SET value=$1 WHERE tenant_id='native-provider-view' AND key IN ('llm.default.model','llm.sempai.model','embedding.model')",&[&generation]).await.unwrap();
            transaction.commit().await.unwrap();
            tokio::task::yield_now().await;
        }
    });
    for _ in 0..96 {
        let snapshot = service.snapshot(caller.clone()).await.unwrap();
        let definition_generation = &snapshot.providers[0].description;
        assert_eq!(
            snapshot.kohai_active.as_ref().unwrap().model.as_ref(),
            Some(definition_generation)
        );
        assert_eq!(
            snapshot.sempai_active.as_ref().unwrap().model.as_ref(),
            Some(definition_generation)
        );
        assert_eq!(
            snapshot.embedding_active.as_ref().unwrap().model.as_ref(),
            Some(definition_generation)
        );
    }
    writer.await.unwrap();
    let final_snapshot = service.snapshot(caller.clone()).await.unwrap();
    assert_eq!(final_snapshot.providers[0].description, "generation64");
    crate::db_config::save_config_changes(
        &rig.pool,
        tenant,
        &[("llm.default.provider_id", Some("missing-selected-provider"))],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    assert!(
        service.snapshot(caller.clone()).await.is_err(),
        "a named missing provider is not silently unconfigured"
    );
    crate::db_config::save_config_changes(
        &rig.pool,
        tenant,
        &[("llm.default.provider_id", Some("legacy-provider"))],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let client = rig.pool.get().await.unwrap();
    client
        .batch_execute("ALTER TABLE brassclaw_llm_providers RENAME TO unavailable_view_providers")
        .await
        .unwrap();
    let unavailable = service.snapshot(caller).await;
    client
        .batch_execute("ALTER TABLE unavailable_view_providers RENAME TO brassclaw_llm_providers")
        .await
        .unwrap();
    assert!(unavailable.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_global_monty_live_model_selection_retains_inflight_exchange() {
    use axum::{Json, Router, extract::State, http::Uri, routing::post};
    use secrecy::ExposeSecret;
    struct EndpointState {
        observations: tokio::sync::Mutex<Vec<(String, String)>>,
        hold_first: std::sync::atomic::AtomicBool,
        started: tokio::sync::Notify,
        release: tokio::sync::Notify,
    }
    async fn endpoint(
        State(state): State<Arc<EndpointState>>,
        uri: Uri,
        Json(body): Json<serde_json::Value>,
    ) -> Json<serde_json::Value> {
        let model = body["model"].as_str().unwrap().to_owned();
        let path = uri.path().to_owned();
        state
            .observations
            .lock()
            .await
            .push((path.clone(), model.clone()));
        if state
            .hold_first
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            state.started.notify_one();
            state.release.notified().await;
        }
        Json(
            serde_json::json!({"id":"nearainswer", "object":"chat.completion", "created":1, "model":model,
            "choices":[{"index":0,"message":{"role":"assistant","content":format!("selected {model} at {path}")},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":1,"completion_tokens":2,"total_tokens":3}}),
        )
    }
    let state = Arc::new(EndpointState {
        observations: tokio::sync::Mutex::new(Vec::new()),
        hold_first: std::sync::atomic::AtomicBool::new(true),
        started: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let listener = native_http::bind_loopback().await.unwrap();
    let address = listener.local_addr().unwrap();
    let shutdown = tokio_util::sync::CancellationToken::new();
    let server_stop = shutdown.clone();
    let app = Router::new()
        .route("/a/v1/chat/completions", post(endpoint))
        .route("/b/v1/chat/completions", post(endpoint))
        .with_state(state.clone());
    let server = tokio::spawn(async move {
        native_http::serve(listener, app, server_stop.cancelled_owned())
            .await
            .unwrap();
    });
    let home = tempfile::tempdir().unwrap();
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db_guard = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let tenant = "native-live-model-selection";
    let repo = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), tenant);
    for (id, path, default_model) in [
        ("nearai", "a", "boot-model"),
        ("openai_codex", "b", "provider-b-default"),
    ] {
        let definition = serde_json::from_value(serde_json::json!({"id":id,"aliases":[],"protocol":"open_ai_completions",
            "default_base_url":format!("http://{address}/{path}/v1"),"base_url_env":null,"base_url_required":true,"api_key_env":null,"api_key_required":false,
            "model_env":"UNUSED_NATIVE_LIVE_MODEL", "default_model":default_model,"description":"Native live selection fixture", "extra_headers_env":null,
            "unsupported_params":[],"setup":null,"token_budget":null,"context_window_tokens":131072,"cache_retention":null})).unwrap();
        repo.upsert(definition).await.unwrap();
    }
    crate::db_config::save_config_changes(
        &rig.pool,
        tenant,
        &[
            ("llm.default.provider_id", Some("nearai")),
            ("llm.default.model", Some("boot-model")),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let input = RebornRuntimeInput::from_services(
        rig.build_input("native-live-owner", home.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )
    .with_identity(RebornRuntimeIdentity {
        tenant_id: tenant.into(),
        agent_id: "nearaigent".into(),
        source_binding_id: "native-live-source".into(),
        reply_target_binding_id: "native-live-reply".into(),
    })
    .with_poll_settings(PollSettings {
        interval: Duration::from_millis(10),
        max_total: RUNTIME_SEND_TIMEOUT,
    });
    let runtime = Arc::new(Box::pin(build_reborn_runtime(input)).await.unwrap());
    let bundle = crate::build_webui_services(runtime.clone(), None)
        .await
        .unwrap();
    let caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        brassclaw_host_api::TenantId::new(tenant).unwrap(),
        brassclaw_host_api::UserId::new("native-live-owner").unwrap(),
        None,
        None,
    );
    let credentials = runtime.services().llm_key_store();
    let request = |id: &str, revision: u64, name: &str, key: &str, set_active: bool| {
        serde_json::from_value::<brassclaw_product_workflow::UpsertLlmProviderRequest>(
            serde_json::json!({
                "id":id, "expected_revision":revision, "name":name, "adapter":"open_ai_completions",
                "base_url":format!("http://{address}/{}/v1",if id=="nearai" {"a"} else {"b"}),
                "default_model":if id=="nearai" {"boot-model"} else {"provider-b-default"},
                "api_key":key,"set_active":set_active,"model":"must-not-be-published-on-failure"
            }),
        )
        .unwrap()
    };
    let before = repo.get_revision("nearai", false).await.unwrap().unwrap();
    let edited = bundle
        .api
        .upsert_llm_provider(
            caller.clone(),
            request(
                "nearai",
                before.revision,
                "Coordinated setup edit",
                "retained-native-setup-key",
                false,
            ),
        )
        .await
        .unwrap();
    let selected = edited
        .providers
        .iter()
        .find(|row| row.id == "nearai")
        .unwrap();
    assert_eq!(selected.revision, before.revision + 1);
    assert_eq!(
        credentials
            .read("nearai")
            .await
            .unwrap()
            .unwrap()
            .expose_secret(),
        "retained-native-setup-key"
    );
    assert!(
        bundle
            .api
            .upsert_llm_provider(
                caller.clone(),
                request(
                    "nearai",
                    before.revision,
                    "Stale edit",
                    "must-not-replace-retained-key",
                    false
                )
            )
            .await
            .is_err()
    );
    let observer = rig.pool.get().await.unwrap();
    observer.batch_execute("CREATE FUNCTION reject_atomic_provider_history() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.tenant_id='native-live-model-selection' AND NEW.provider_id='nearai' THEN RAISE EXCEPTION 'deliberate history rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_atomic_provider_history BEFORE INSERT ON brassclaw_llm_provider_revisions FOR EACH ROW EXECUTE FUNCTION reject_atomic_provider_history();").await.unwrap();
    assert!(
        bundle
            .api
            .upsert_llm_provider(
                caller.clone(),
                request(
                    "nearai",
                    selected.revision,
                    "Rejected history",
                    "must-not-publish-rejected-key",
                    false
                )
            )
            .await
            .is_err()
    );
    observer.batch_execute("DROP TRIGGER reject_atomic_provider_history ON brassclaw_llm_provider_revisions; DROP FUNCTION reject_atomic_provider_history(); CREATE FUNCTION reject_atomic_provider_selection() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.tenant_id='native-live-model-selection' AND NEW.key='llm.default.provider_id' THEN RAISE EXCEPTION 'deliberate role rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_atomic_provider_selection BEFORE INSERT OR UPDATE ON brassclaw_config FOR EACH ROW EXECUTE FUNCTION reject_atomic_provider_selection();").await.unwrap();
    let b_before = repo
        .get_revision("openai_codex", false)
        .await
        .unwrap()
        .unwrap();
    assert!(
        bundle
            .api
            .upsert_llm_provider(
                caller.clone(),
                request(
                    "openai_codex",
                    b_before.revision,
                    "Rejected role",
                    "must-rollback-new-secret",
                    true
                )
            )
            .await
            .is_err()
    );
    observer.batch_execute("DROP TRIGGER reject_atomic_provider_selection ON brassclaw_config; DROP FUNCTION reject_atomic_provider_selection();").await.unwrap();
    assert!(credentials.read("openai_codex").await.unwrap().is_none());
    let b_after = repo
        .get_revision("openai_codex", false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(b_after.revision, b_before.revision);
    assert_eq!(
        b_after.definition.description,
        b_before.definition.description
    );
    let a_after = repo.get_revision("nearai", false).await.unwrap().unwrap();
    assert_eq!(a_after.revision, selected.revision);
    assert_eq!(a_after.definition.description, "Coordinated setup edit");
    assert_eq!(
        credentials
            .read("nearai")
            .await
            .unwrap()
            .unwrap()
            .expose_secret(),
        "retained-native-setup-key"
    );
    drop(observer);
    let conversation = runtime.new_conversation().await.unwrap();
    let first_runtime = runtime.clone();
    let first_conversation = conversation.clone();
    let first = tokio::spawn(async move {
        first_runtime
            .send_user_message(
                &first_conversation,
                "ordinary opaque model selection probe before edit",
            )
            .await
    });
    let started = tokio::time::timeout(Duration::from_secs(5), state.started.notified()).await;
    if started.is_err() {
        state.release.notify_one();
    }
    assert!(
        started.is_ok(),
        "ordinary Monty task reached the actual provider endpoint"
    );
    let selection = bundle
        .api
        .set_active_llm(
            brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
                brassclaw_host_api::TenantId::new(tenant).unwrap(),
                brassclaw_host_api::UserId::new("native-live-owner").unwrap(),
                None,
                None,
            ),
            brassclaw_product_workflow::SetActiveLlmRequest {
                provider_id: "openai_codex".into(),
                model: Some("selected-live-model".into()),
                role: Some(brassclaw_product_workflow::ProviderRole::Kohai),
            },
        )
        .await;
    state.release.notify_one();
    let first = first.await.unwrap().unwrap();
    selection.unwrap();
    let second = runtime
        .send_user_message(
            &conversation,
            "ordinary opaque model selection probe after edit",
        )
        .await
        .unwrap();
    let b_revision = repo
        .get_revision("openai_codex", false)
        .await
        .unwrap()
        .unwrap()
        .revision;
    assert!(
        bundle
            .api
            .delete_llm_provider(caller.clone(), "openai_codex".into(), b_revision + 1)
            .await
            .is_err()
    );
    assert!(repo.get("openai_codex").await.unwrap().is_some());
    let removed = bundle
        .api
        .delete_llm_provider(caller.clone(), "openai_codex".into(), b_revision)
        .await
        .unwrap();
    assert!(removed.active.is_none());
    assert!(!removed.providers.iter().any(|row| row.id == "openai_codex"));
    let retained = removed
        .inactive_providers
        .iter()
        .find(|row| row.id == "openai_codex")
        .unwrap();
    assert_eq!(retained.revision, b_revision + 1);
    let mut restore = request(
        "openai_codex",
        retained.revision,
        "Explicitly restored provider",
        "unused",
        false,
    );
    restore.api_key = None;
    restore.model = None;
    let restored = bundle
        .api
        .upsert_llm_provider(caller.clone(), restore)
        .await
        .unwrap();
    assert!(
        restored.active.is_none(),
        "reactivation never restores an old role selection"
    );
    assert!(
        !restored
            .inactive_providers
            .iter()
            .any(|row| row.id == "openai_codex")
    );
    assert_eq!(
        restored
            .providers
            .iter()
            .filter(|row| row.id == "openai_codex")
            .count(),
        1
    );
    assert_eq!(
        restored
            .providers
            .iter()
            .find(|row| row.id == "openai_codex")
            .unwrap()
            .revision,
        retained.revision + 1
    );
    assert!(credentials.read("openai_codex").await.unwrap().is_none());
    assert_eq!(
        credentials
            .read("nearai")
            .await
            .unwrap()
            .unwrap()
            .expose_secret(),
        "retained-native-setup-key"
    );
    bundle
        .api
        .set_active_llm(
            caller.clone(),
            brassclaw_product_workflow::SetActiveLlmRequest {
                provider_id: "openai_codex".into(),
                model: Some("selected-live-model".into()),
                role: None,
            },
        )
        .await
        .unwrap();
    let third = runtime
        .send_user_message(
            &conversation,
            "ordinary opaque model probe after explicit reactivation",
        )
        .await
        .unwrap();
    let mut outcomes = Vec::new();
    for reply in [&first, &second, &third] {
        let row = rig.pool.get().await.unwrap().query_one("SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'",&[&reply.run_id.as_uuid()]).await.unwrap();
        outcomes.push(row.get::<_, serde_json::Value>(0));
    }
    drop(bundle);
    Arc::try_unwrap(runtime)
        .unwrap_or_else(|_| panic!("runtime owner retained after completed exchanges"))
        .shutdown()
        .await
        .unwrap();
    shutdown.cancel();
    server.await.unwrap();
    assert_eq!(first.status, TurnStatus::Completed);
    assert_eq!(
        first.text.as_deref(),
        Some("selected boot-model at /a/v1/chat/completions")
    );
    assert_eq!(second.status, TurnStatus::Completed);
    assert_eq!(
        second.text.as_deref(),
        Some("selected selected-live-model at /b/v1/chat/completions")
    );
    assert_eq!(third.status, TurnStatus::Completed);
    assert_eq!(third.text, second.text);
    assert_eq!(
        *state.observations.lock().await,
        vec![
            ("/a/v1/chat/completions".into(), "boot-model".into()),
            (
                "/b/v1/chat/completions".into(),
                "selected-live-model".into()
            ),
            (
                "/b/v1/chat/completions".into(),
                "selected-live-model".into()
            )
        ]
    );
    assert!(outcomes[0]["execution"]["root"]["vm_id"].is_string());
    assert_eq!(
        outcomes[0]["execution"]["root"]["vm_id"],
        outcomes[1]["execution"]["root"]["vm_id"]
    );
    assert_eq!(
        outcomes[0]["execution"]["root"]["vm_id"],
        outcomes[2]["execution"]["root"]["vm_id"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_llm_response_cache_zero_capacity_and_completion_ttl() {
    use axum::{Json, Router, extract::State, routing::post};
    use brassclaw_llm::{CachedProvider, LlmProvider, ResponseCacheConfig};
    struct EndpointState {
        calls: std::sync::atomic::AtomicUsize,
        delay_millis: std::sync::atomic::AtomicU64,
    }
    async fn endpoint(
        State(state): State<Arc<EndpointState>>,
        Json(body): Json<serde_json::Value>,
    ) -> Json<serde_json::Value> {
        state
            .calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(
            state.delay_millis.load(std::sync::atomic::Ordering::SeqCst),
        ))
        .await;
        Json(
            serde_json::json!({"id":"native-cache-answer","object":"chat.completion","created":1,"model":body["model"],"choices":[{"index":0,"message":{"role":"assistant","content":"protocol answer"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":2,"total_tokens":3}}),
        )
    }
    let state = Arc::new(EndpointState {
        calls: std::sync::atomic::AtomicUsize::new(0),
        delay_millis: std::sync::atomic::AtomicU64::new(0),
    });
    let listener = native_http::bind_loopback().await.unwrap();
    let address = listener.local_addr().unwrap();
    let shutdown = tokio_util::sync::CancellationToken::new();
    let server_stop = shutdown.clone();
    let app = Router::new()
        .route("/v1/chat/completions", post(endpoint))
        .with_state(state.clone());
    let server = tokio::spawn(async move {
        native_http::serve(listener, app, server_stop.cancelled_owned())
            .await
            .unwrap();
    });
    let raw = brassclaw_llm::create_registry_provider(
        &brassclaw_llm::RegistryProviderConfig::generic(
            brassclaw_llm::ProviderProtocol::OpenAiCompletions,
            "native-cache-http",
            None,
            format!("http://{address}/v1"),
            "native-cache-model",
        ),
        30,
    )
    .unwrap();
    let request = || brassclaw_llm::CompletionRequest {
        messages: vec![brassclaw_llm::ChatMessage::user("native cache request")],
        model: None,
        max_tokens: Some(16),
        temperature: None,
        stop_sequences: None,
        metadata: Default::default(),
    };
    for config in [
        ResponseCacheConfig {
            ttl: Duration::from_secs(60),
            max_entries: 0,
        },
        ResponseCacheConfig {
            ttl: Duration::ZERO,
            max_entries: 100,
        },
    ] {
        let cached = CachedProvider::new(raw.clone(), config);
        for _ in 0..2 {
            assert_eq!(
                cached.complete(request()).await.unwrap().content,
                "protocol answer"
            );
        }
        assert!(cached.is_empty());
    }
    assert_eq!(state.calls.load(std::sync::atomic::Ordering::SeqCst), 4);
    state
        .delay_millis
        .store(2100, std::sync::atomic::Ordering::SeqCst);
    let cached = CachedProvider::new(
        raw.clone(),
        ResponseCacheConfig {
            ttl: Duration::from_secs(2),
            max_entries: 100,
        },
    );
    let first = cached.complete(request()).await;
    let second = cached.complete(request()).await;
    let first = first.unwrap();
    let second = second.unwrap();
    assert_eq!(first.content, "protocol answer");
    assert_eq!(second.content, first.content);
    assert_eq!((first.input_tokens, first.output_tokens), (1, 2));
    assert_eq!(
        (second.input_tokens, second.output_tokens),
        (0, 0),
        "a cache hit cannot rebill preceding provider usage"
    );
    assert_eq!(
        state.calls.load(std::sync::atomic::Ordering::SeqCst),
        5,
        "external latency does not consume the answer's cache TTL"
    );
    assert_eq!(cached.total_hits(), 1);
    state
        .delay_millis
        .store(0, std::sync::atomic::Ordering::SeqCst);
    let isolated = CachedProvider::new(
        raw,
        ResponseCacheConfig {
            ttl: Duration::from_secs(60),
            max_entries: 100,
        },
    );
    let mut scoped = request();
    scoped.metadata.insert("owner".into(), "first-owner".into());
    scoped
        .metadata
        .insert("prefix_generation".into(), "generation-a".into());
    isolated.complete(scoped.clone()).await.unwrap();
    isolated.complete(scoped.clone()).await.unwrap();
    let mut other_owner = scoped.clone();
    other_owner
        .metadata
        .insert("owner".into(), "second-owner".into());
    isolated.complete(other_owner).await.unwrap();
    let mut reordered = scoped.clone();
    reordered.metadata.clear();
    reordered
        .metadata
        .insert("prefix_generation".into(), "generation-a".into());
    reordered
        .metadata
        .insert("owner".into(), "first-owner".into());
    isolated.complete(reordered).await.unwrap();
    scoped
        .metadata
        .insert("prefix_generation".into(), "generation-b".into());
    isolated.complete(scoped.clone()).await.unwrap();
    let mut different_stops = scoped.clone();
    scoped.stop_sequences = Some(vec!["a\0b".into(), "c".into()]);
    different_stops.stop_sequences = Some(vec!["a".into(), "b\0c".into()]);
    isolated.complete(scoped).await.unwrap();
    isolated.complete(different_stops).await.unwrap();
    assert_eq!(
        state.calls.load(std::sync::atomic::Ordering::SeqCst),
        10,
        "owner, prefix generation and list boundaries must not share cache entries"
    );
    assert_eq!(
        isolated.total_hits(),
        2,
        "map insertion order does not alter a request's identity"
    );
    shutdown.cancel();
    server.await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pg_credential_sessions_fence_last_use_and_reissuance() {
    use base64::Engine as _;
    use brassclaw_host_api::{
        CapabilityId, ExtensionId, NetworkMethod, ResourceScope, SecretHandle, TenantId, UserId,
    };
    use brassclaw_secrets::{
        CredentialAccount, CredentialAccountId, CredentialAccountStatus, CredentialAccountStore,
        CredentialBrokerError, CredentialPathPolicy, CredentialSessionRequest,
        CredentialSessionStore, CredentialTargetPolicy, InMemoryCredentialBroker,
        PgCredentialBroker, RedactedJson, SecretsCrypto, credential_account_aad,
        credential_session_aad,
    };
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let master = "native-credential-session-master-key";
    let store = Arc::new(
        PgCredentialBroker::new(
            rig.pool.as_ref().clone(),
            secrecy::SecretString::from(master),
            "physical-broker-a",
        )
        .unwrap(),
    );
    let owner = ResourceScope {
        tenant_id: TenantId::new("logical-broker").unwrap(),
        user_id: UserId::new("logical-owner").unwrap(),
        ..ResourceScope::system()
    };
    let account = CredentialAccount {
        scope: owner.clone(),
        id: CredentialAccountId::new("native_credential").unwrap(),
        provider_or_extension_id: ExtensionId::new("openai").unwrap(),
        label: "Native broker fixture".into(),
        status: CredentialAccountStatus::Active,
        secret_handles: vec![SecretHandle::new("native-api-key").unwrap()],
        allowed_targets: vec![CredentialTargetPolicy {
            scheme: "https".into(),
            host: "api.example.com".into(),
            port: None,
            path: CredentialPathPolicy::Prefix("/v1".into()),
            methods: vec![NetworkMethod::Get],
        }],
        redacted_metadata: RedactedJson::new(serde_json::json!({})),
        updated_at: chrono::Utc::now(),
    };
    // Use the real issuer to construct a validated session, then exercise the
    // PostgreSQL persistence/consumption owner, not a substitute store.
    let issuer = InMemoryCredentialBroker::new();
    issuer.put_account(account.clone()).unwrap();
    store.put_account(account.clone()).await.unwrap();
    let request = CredentialSessionRequest {
        scope: owner.clone(),
        invocation_id: owner.invocation_id,
        capability_id: CapabilityId::new("openai.chat").unwrap(),
        extension_id: account.provider_or_extension_id.clone(),
        account_id: account.id.clone(),
        method: NetworkMethod::Get,
        url: "https://api.example.com/v1/models".into(),
        expires_at: Some(chrono::Utc::now() + chrono::Duration::seconds(60)),
        max_uses: Some(1),
    };
    let session = issuer.create_session(request.clone()).unwrap();
    store.issue_session(session.clone()).await.unwrap();
    let mut client = rig.pool.get().await.unwrap();
    let held = client.transaction().await.unwrap();
    held.query_one("SELECT scope FROM brassclaw_secrets WHERE tenant_id = 'physical-broker-a' AND name = 'session' FOR UPDATE", &[]).await.unwrap();
    let first_store = store.clone();
    let first_owner = owner.clone();
    let id = session.correlation_id();
    let first = tokio::spawn(async move {
        first_store
            .consume_session_use(&first_owner, id, chrono::Utc::now())
            .await
    });
    let second_store = store.clone();
    let second_owner = owner.clone();
    let second = tokio::spawn(async move {
        second_store
            .consume_session_use(&second_owner, id, chrono::Utc::now())
            .await
    });
    let observer = rig.pool.get().await.unwrap();
    let both_blocked = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let blocked: i64 = observer.query_one("SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() AND wait_event_type = 'Lock' AND query LIKE '%brassclaw_secrets%'", &[]).await.unwrap().get(0);
            if blocked >= 2 { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await;
    held.commit().await.unwrap();
    let first = first.await.unwrap();
    let second = second.await.unwrap();
    assert!(
        both_blocked.is_ok(),
        "both consumers reached the PostgreSQL row lock"
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert!(matches!(
        first.as_ref().err().or(second.as_ref().err()),
        Some(CredentialBrokerError::SessionUseLimitExceeded { .. })
    ));
    assert!(matches!(
        store.validate_session(&owner, id, chrono::Utc::now()).await,
        Err(CredentialBrokerError::SessionUseLimitExceeded { .. })
    ));
    store.issue_session(session.clone()).await.unwrap();
    assert!(matches!(
        store
            .consume_session_use(&owner, id, chrono::Utc::now())
            .await,
        Err(CredentialBrokerError::SessionUseLimitExceeded { .. })
    ));

    let copied = PgCredentialBroker::new(
        rig.pool.as_ref().clone(),
        secrecy::SecretString::from(master),
        "physical-broker-b",
    )
    .unwrap();
    observer.execute("INSERT INTO brassclaw_secrets (tenant_id, scope, name, ciphertext, key_version) SELECT 'physical-broker-b', scope, name, ciphertext, key_version FROM brassclaw_secrets WHERE tenant_id = 'physical-broker-a'", &[]).await.unwrap();
    assert!(copied.get_account(&owner, &account.id).await.is_err());
    assert!(copied.accounts_for_scope(&owner).await.is_err());
    assert!(copied.get_session(&owner, id).await.is_err());
    assert!(
        copied
            .consume_session_use(&owner, id, chrono::Utc::now())
            .await
            .is_err()
    );
    let wrong_key = PgCredentialBroker::new(
        rig.pool.as_ref().clone(),
        secrecy::SecretString::from("different-credential-session-master-key"),
        "physical-broker-a",
    )
    .unwrap();
    assert!(wrong_key.put_account(account.clone()).await.is_err());
    assert!(wrong_key.issue_session(session).await.is_err());
    assert_eq!(
        store
            .get_account(&owner, &account.id)
            .await
            .unwrap()
            .unwrap(),
        account
    );

    let mut legacy_account = account.clone();
    legacy_account.id = CredentialAccountId::new("legacy_credential").unwrap();
    issuer.put_account(legacy_account.clone()).unwrap();
    let crypto = SecretsCrypto::new(secrecy::SecretString::from(master)).unwrap();
    let legacy_bytes = serde_json::to_vec(&serde_json::json!({"account":legacy_account})).unwrap();
    let (ciphertext, salt) = crypto
        .encrypt(
            &legacy_bytes,
            &credential_account_aad(&owner, &legacy_account.id),
        )
        .unwrap();
    let encoded = base64::engine::general_purpose::STANDARD
        .encode([salt.as_slice(), ciphertext.as_slice()].concat());
    let legacy_name = format!(
        "tenant:{}/user:{}/agent:/project:",
        owner.tenant_id, owner.user_id
    );
    let legacy_key = format!("credential-account:{}", legacy_account.id.as_str());
    observer.execute("INSERT INTO brassclaw_secrets (tenant_id, scope, name, ciphertext, key_version) VALUES ('physical-broker-a', $1, $2, $3, 1)", &[&legacy_key, &legacy_name, &encoded]).await.unwrap();
    assert_eq!(
        store
            .get_account(&owner, &legacy_account.id)
            .await
            .unwrap()
            .unwrap(),
        legacy_account
    );
    store.put_account(legacy_account.clone()).await.unwrap();
    assert!(observer.query_one("SELECT ciphertext FROM brassclaw_secrets WHERE tenant_id = 'physical-broker-a' AND scope = $1", &[&legacy_key]).await.unwrap().get::<_, String>(0).starts_with("v2:"));
    let legacy_session = issuer
        .create_session(CredentialSessionRequest {
            account_id: legacy_account.id.clone(),
            max_uses: Some(2),
            ..request.clone()
        })
        .unwrap();
    store.issue_session(legacy_session.clone()).await.unwrap();
    let legacy_session_key: String = observer.query_one("SELECT scope FROM brassclaw_secrets WHERE tenant_id = 'physical-broker-a' AND name = 'session' ORDER BY created_at DESC LIMIT 1", &[]).await.unwrap().get(0);
    let legacy_payload = serde_json::to_vec(&serde_json::json!({
        "scope":legacy_session.scope(), "invocation_id":legacy_session.invocation_id(), "capability_id":legacy_session.capability_id(), "extension_id":legacy_session.extension_id(), "account_id":legacy_session.account_id(), "secret_handles":legacy_session.secret_handles(), "allowed_targets":legacy_session.allowed_targets(), "expires_at":legacy_session.expires_at(), "max_uses":legacy_session.max_uses(), "correlation_id":legacy_session_key.strip_prefix("credential-session:").unwrap(), "uses":0
    })).unwrap();
    let (ciphertext, salt) = crypto
        .encrypt(
            &legacy_payload,
            &credential_session_aad(&owner, legacy_session.correlation_id()),
        )
        .unwrap();
    let encoded = base64::engine::general_purpose::STANDARD
        .encode([salt.as_slice(), ciphertext.as_slice()].concat());
    observer.execute("UPDATE brassclaw_secrets SET ciphertext = $2 WHERE tenant_id = 'physical-broker-a' AND scope = $1 AND name = 'session'", &[&legacy_session_key, &encoded]).await.unwrap();
    observer.batch_execute("CREATE FUNCTION reject_native_credential_use() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF OLD.tenant_id = 'physical-broker-a' AND OLD.name = 'session' THEN RAISE EXCEPTION 'deliberate credential consumption rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_native_credential_use BEFORE UPDATE ON brassclaw_secrets FOR EACH ROW EXECUTE FUNCTION reject_native_credential_use();").await.unwrap();
    let rejected = store
        .consume_session_use(&owner, legacy_session.correlation_id(), chrono::Utc::now())
        .await;
    observer.batch_execute("DROP TRIGGER reject_native_credential_use ON brassclaw_secrets; DROP FUNCTION reject_native_credential_use();").await.unwrap();
    assert!(rejected.is_err());
    assert_eq!(observer.query_one("SELECT ciphertext FROM brassclaw_secrets WHERE tenant_id = 'physical-broker-a' AND scope = $1", &[&legacy_session_key]).await.unwrap().get::<_, String>(0), encoded);
    store
        .consume_session_use(&owner, legacy_session.correlation_id(), chrono::Utc::now())
        .await
        .unwrap();
    assert!(observer.query_one("SELECT ciphertext FROM brassclaw_secrets WHERE tenant_id = 'physical-broker-a' AND scope = $1", &[&legacy_session_key]).await.unwrap().get::<_, String>(0).starts_with("v2:"));
    store
        .consume_session_use(&owner, legacy_session.correlation_id(), chrono::Utc::now())
        .await
        .unwrap();
    assert!(matches!(
        store
            .consume_session_use(&owner, legacy_session.correlation_id(), chrono::Utc::now())
            .await,
        Err(CredentialBrokerError::SessionUseLimitExceeded { .. })
    ));
    let expired = issuer
        .create_session(CredentialSessionRequest {
            expires_at: Some(chrono::Utc::now() - chrono::Duration::seconds(1)),
            ..request
        })
        .unwrap();
    store.issue_session(expired.clone()).await.unwrap();
    assert!(matches!(
        store
            .consume_session_use(&owner, expired.correlation_id(), chrono::Utc::now())
            .await,
        Err(CredentialBrokerError::SessionExpired { .. })
    ));
    let mut system_account = account;
    system_account.scope = ResourceScope::system();
    issuer.put_account(system_account.clone()).unwrap();
    store.put_account(system_account.clone()).await.unwrap();
    assert_eq!(
        store
            .get_account(&system_account.scope, &system_account.id)
            .await
            .unwrap()
            .unwrap(),
        system_account
    );
    let system_session = issuer
        .create_session(CredentialSessionRequest {
            scope: system_account.scope.clone(),
            invocation_id: system_account.scope.invocation_id,
            capability_id: CapabilityId::new("openai.chat").unwrap(),
            extension_id: system_account.provider_or_extension_id.clone(),
            account_id: system_account.id.clone(),
            method: NetworkMethod::Get,
            url: "https://api.example.com/v1/models".into(),
            expires_at: None,
            max_uses: Some(1),
        })
        .unwrap();
    store.issue_session(system_session.clone()).await.unwrap();
    store
        .consume_session_use(
            &system_account.scope,
            system_session.correlation_id(),
            chrono::Utc::now(),
        )
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_startup_restores_sempai_before_ingress_without_webui() {
    use crate::db_config::{ConfigWriteContext, save_config_changes};
    let home = tempfile::tempdir().unwrap();
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _guard = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let tenant = "native-sempai-boot";
    let repo = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), tenant);
    let definition: brassclaw_llm::ProviderDefinition = serde_json::from_value(serde_json::json!({
        "id": "installed-local-provider", "aliases": [], "protocol": "open_ai_completions",
        "default_base_url": "http://127.0.0.1:8000/v1", "base_url_env": null,
        "base_url_required": true, "api_key_env": null, "api_key_required": false,
        "model_env": "BRASSCLAW_UNUSED_NATIVE_BOOT_MODEL", "default_model": "provider-default",
        "description": "Native stored Sempai startup test", "extra_headers_env": null,
        "unsupported_params": [], "setup": null, "token_budget": null,
        "context_window_tokens": 131072, "cache_retention": null
    }))
    .unwrap();
    repo.upsert(definition).await.unwrap();
    save_config_changes(
        &rig.pool,
        tenant,
        &[
            ("llm.default.provider_id", Some("installed-local-provider")),
            ("llm.default.model", Some("selected-primary-model")),
            ("llm.sempai.provider_id", Some("installed-local-provider")),
            ("llm.sempai.model", Some("selected-sempai-model")),
        ],
        ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let input = || {
        RebornRuntimeInput::from_services(
            rig.build_input("native-sempai-owner", home.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: tenant.into(),
            agent_id: "native-sempai-agent".into(),
            source_binding_id: "native-sempai-source".into(),
            reply_target_binding_id: "native-sempai-reply".into(),
        })
    };
    for boot in 0..2 {
        let runtime = Arc::new(build_reborn_runtime(input()).await.unwrap());
        assert!(runtime.services().readiness.workers.turn_runner);
        assert_eq!(
            runtime
                .llm_reload
                .as_ref()
                .unwrap()
                .reload_handle
                .primary_provider()
                .active_model_name(),
            "selected-primary-model"
        );
        assert_eq!(
            runtime.sempai_swappable().unwrap().pinned_provider().1,
            "selected-sempai-model"
        );
        assert_eq!(
            runtime.interceptor_mode().unwrap().get(),
            brassclaw_interceptor::InterceptorMode::Rerouting
        );
        if boot == 0 {
            let primary = runtime
                .llm_reload
                .as_ref()
                .unwrap()
                .reload_handle
                .primary_provider();
            save_config_changes(
                &rig.pool,
                tenant,
                &[("llm.default.model", Some("pending-explicit-model"))],
                ConfigWriteContext::Operator,
            )
            .await
            .unwrap();
            let sempai = runtime.sempai_swappable().unwrap();
            let admitted_sempai = sempai.pinned_provider().0;
            let bundle = crate::build_webui_services(runtime.clone(), None)
                .await
                .unwrap();
            assert_eq!(primary.active_model_name(), "selected-primary-model");
            assert!(Arc::ptr_eq(&admitted_sempai, &sempai.pinned_provider().0));
            bundle
                .api
                .set_active_llm(
                    brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
                        brassclaw_host_api::TenantId::new(tenant).unwrap(),
                        brassclaw_host_api::UserId::new("native-sempai-owner").unwrap(),
                        None,
                        None,
                    ),
                    brassclaw_product_workflow::SetActiveLlmRequest {
                        provider_id: "installed-local-provider".into(),
                        model: Some("webui-live-primary-model".into()),
                        role: Some(brassclaw_product_workflow::ProviderRole::Kohai),
                    },
                )
                .await
                .unwrap();
            assert_eq!(primary.active_model_name(), "webui-live-primary-model");
            assert!(Arc::ptr_eq(&admitted_sempai, &sempai.pinned_provider().0));
            bundle
                .api
                .set_active_llm(
                    brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
                        brassclaw_host_api::TenantId::new(tenant).unwrap(),
                        brassclaw_host_api::UserId::new("native-sempai-owner").unwrap(),
                        None,
                        None,
                    ),
                    brassclaw_product_workflow::SetActiveLlmRequest {
                        provider_id: "installed-local-provider".into(),
                        model: Some("selected-primary-model".into()),
                        role: Some(brassclaw_product_workflow::ProviderRole::Kohai),
                    },
                )
                .await
                .unwrap();
            drop(bundle);
        }
        Arc::try_unwrap(runtime)
            .unwrap_or_else(|_| panic!("WebUI retains its runtime owner"))
            .shutdown()
            .await
            .unwrap();
    }
    let client = rig.pool.get().await.unwrap();
    client
        .batch_execute("ALTER TABLE brassclaw_llm_providers RENAME TO unavailable_boot_providers")
        .await
        .unwrap();
    let unavailable = build_reborn_runtime(input()).await;
    client
        .batch_execute("ALTER TABLE unavailable_boot_providers RENAME TO brassclaw_llm_providers")
        .await
        .unwrap();
    assert!(matches!(
        unavailable,
        Err(crate::runtime::RebornRuntimeError::LlmProvider(_))
    ));
    let saved = crate::db_config::list_config_keys(&rig.pool, tenant)
        .await
        .unwrap();
    assert!(
        saved
            .iter()
            .any(|(key, value)| key == "llm.default.model" && value == "selected-primary-model")
    );
    assert!(repo.delete("installed-local-provider").await.unwrap());
    let deactivated = build_reborn_runtime(input()).await.unwrap();
    assert_eq!(
        deactivated
            .llm_reload
            .as_ref()
            .unwrap()
            .reload_handle
            .primary_provider()
            .active_model_name(),
        "unconfigured"
    );
    assert_eq!(
        deactivated.sempai_swappable().unwrap().pinned_provider().1,
        "unconfigured"
    );
    deactivated.shutdown().await.unwrap();
    assert!(
        repo.get("installed-local-provider")
            .await
            .unwrap()
            .is_none()
    );
    let retained = crate::db_config::list_config_keys(&rig.pool, tenant)
        .await
        .unwrap();
    assert!(
        !retained
            .iter()
            .any(|(key, _)| key.starts_with("llm.sempai.") || key.starts_with("llm.default."))
    );
    let runtime = build_reborn_runtime(input()).await.unwrap();
    assert_eq!(
        runtime.sempai_swappable().unwrap().pinned_provider().1,
        "unconfigured"
    );
    assert_eq!(
        runtime.interceptor_mode().unwrap().get(),
        brassclaw_interceptor::InterceptorMode::Routing
    );
    assert!(
        repo.get("installed-local-provider")
            .await
            .unwrap()
            .is_none()
    );
    runtime.shutdown().await.unwrap();
}

/// Real PostgreSQL selection/secret reads and real HTTP requests against a
/// controlled protocol endpoint. This verifies transport contracts, not model
/// quality or deployment-provider inference.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_embedding_catalogue_credentials_endpoints_and_errors() {
    use crate::db_config::{ConfigWriteContext, save_config_changes};
    use axum::{
        Json, Router,
        extract::State,
        http::{HeaderMap, Uri},
        response::IntoResponse,
        routing::post,
    };
    type Observation = (String, Option<String>, serde_json::Value);
    type Observations = Arc<tokio::sync::Mutex<Vec<Observation>>>;
    async fn embedding_endpoint(
        State(observations): State<Observations>,
        uri: Uri,
        headers: HeaderMap,
        Json(body): Json<serde_json::Value>,
    ) -> axum::response::Response {
        observations.lock().await.push((
            uri.path().to_string(),
            headers
                .get("authorization")
                .map(|value| value.to_str().unwrap().to_string()),
            body.clone(),
        ));
        let dimension = if body["model"] == "native-custom-vector-model" {
            3
        } else {
            1536
        };
        let inputs = body["input"].as_array().unwrap();
        let first = inputs[0].as_str().unwrap();
        if first.starts_with("delay-fixture:") {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        }
        if first.starts_with("redirect-fixture:") {
            return (
                axum::http::StatusCode::TEMPORARY_REDIRECT,
                [("location", "/custom/v1/embeddings")],
            )
                .into_response();
        }
        if first.starts_with("capacity-fixture:") {
            let payload = serde_json::to_vec(&serde_json::json!({
                "padding":"x".repeat(4096),
                "data":[{"index":0,"embedding":[1.0,1.0,1.0]}],
                "embeddings":[[1.0,1.0,1.0]]
            }))
            .unwrap();
            let chunks = payload
                .chunks(61)
                .map(|chunk| Ok::<_, std::io::Error>(axum::body::Bytes::copy_from_slice(chunk)))
                .collect::<Vec<_>>();
            return axum::body::Body::from_stream(futures::stream::iter(chunks)).into_response();
        }
        for (marker, status) in [
            ("auth-error-fixture:", axum::http::StatusCode::UNAUTHORIZED),
            (
                "rate-error-fixture:",
                axum::http::StatusCode::TOO_MANY_REQUESTS,
            ),
            (
                "service-error-fixture:",
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ] {
            if first.starts_with(marker) {
                return (
                    status,
                    [("retry-after", "7")],
                    Json(serde_json::json!({"error":format!(
                        "native-sensitive-service-payload:{first}:native-stored-embedding-key"
                    )})),
                )
                    .into_response();
            }
        }
        if first.starts_with("malformed-response-fixture:") {
            return Json(serde_json::json!({
                "data":format!("native-sensitive-service-payload:{first}"),
                "embeddings":format!("native-sensitive-service-payload:{first}")
            }))
            .into_response();
        }
        if inputs
            .iter()
            .any(|input| input.as_str().unwrap().starts_with("model-limit-fixture:"))
        {
            return axum::response::IntoResponse::into_response((
                axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                Json(serde_json::json!({"error":"model input limit exceeded"})),
            ));
        }
        if uri.path().ends_with("/api/embed") {
            Json(serde_json::json!({ "embeddings": inputs.iter().enumerate()
                .map(|(index, _)| vec![(index + 1) as f32; dimension]).collect::<Vec<_>>() }))
            .into_response()
        } else {
            Json(serde_json::json!({ "data": inputs.iter().enumerate().rev()
                .map(|(index, _)| serde_json::json!({ "index": index, "embedding": vec![(index + 1) as f32; dimension] }))
                .collect::<Vec<_>>() })).into_response()
        }
    }
    let observations: Observations = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = native_http::bind_loopback().await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new()
        .route("/custom/v1/embeddings", post(embedding_endpoint))
        .route("/custom/api/embed", post(embedding_endpoint))
        .with_state(Arc::clone(&observations));
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        native_http::serve(listener, router, async {
            let _ = stopped.await;
        })
        .await
        .unwrap();
    });
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let storage = tempfile::tempdir().unwrap();
    let missing_storage = tempfile::tempdir().unwrap();
    let master_key = "native-embedding-retained-master-key";
    std::fs::write(
        storage.path().join(".reborn-local-dev-secrets-master-key"),
        master_key,
    )
    .unwrap();
    let data_owner = "native-embedding-data-owner";
    let secret_owner = "native-embedding-secret-owner";
    let pool = rig.pool.as_ref();
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            pool.clone(),
            secrecy::SecretString::from(master_key),
            secret_owner,
        )
        .unwrap(),
    ));
    let repo = crate::PgProviderRepo::new(pool.clone(), data_owner);
    let resolve = |storage_path: std::path::PathBuf| async move {
        crate::factory::resolve_pg_embedding_provider_pub(
            pool,
            data_owner,
            &storage_path,
            secret_owner,
        )
        .await
    };
    assert!(
        resolve(missing_storage.path().into())
            .await
            .unwrap()
            .is_none()
    );
    for (id, protocol, key_required, endpoint) in [
        (
            "native-openai",
            "open_ai_completions",
            true,
            format!("http://{address}/custom/v1/"),
        ),
        (
            "native-nearai",
            "nearai",
            true,
            format!("http://{address}/custom/v1"),
        ),
        (
            "native-ollama",
            "ollama",
            false,
            format!("http://{address}/custom/"),
        ),
        (
            "native-optional",
            "open_ai_completions",
            false,
            format!("http://{address}/custom/v1"),
        ),
    ] {
        let alias = format!("alias-{id}");
        let definition = serde_json::from_value(serde_json::json!({
            "id": id, "aliases": [alias], "protocol": protocol,
            "default_base_url": endpoint, "base_url_env": null, "base_url_required": true,
            "api_key_env": null, "api_key_required": key_required,
            "model_env": "BRASSCLAW_UNUSED_EMBEDDING_MODEL", "default_model": "provider-default",
            "description": "Native embedding protocol fixture", "extra_headers_env": null,
            "unsupported_params": [], "setup": null, "token_budget": null,
            "context_window_tokens": 131072, "cache_retention": null,
            "embedding_models":{"native-custom-vector-model":{"dimension":3,"cache_max_entries":3}}
        }))
        .unwrap();
        repo.upsert(definition).await.unwrap();
        save_config_changes(
            pool,
            data_owner,
            &[
                ("embedding.provider_id", Some(alias.as_str())),
                ("embedding.model", Some("native-custom-vector-model")),
            ],
            ConfigWriteContext::Operator,
        )
        .await
        .unwrap();
        assert!(resolve(missing_storage.path().into()).await.is_err());
        if key_required {
            assert!(resolve(storage.path().into()).await.is_err());
            keys.put(
                id,
                secrecy::SecretString::from("native-stored-embedding-key"),
            )
            .await
            .unwrap();
            assert!(
                crate::factory::resolve_pg_embedding_provider_pub(
                    pool,
                    data_owner,
                    storage.path(),
                    "wrong-secret-owner",
                )
                .await
                .is_err()
            );
        }
        let provider = resolve(storage.path().into()).await.unwrap().unwrap();
        assert_eq!(provider.model_name(), "native-custom-vector-model");
        assert_eq!(provider.dimension(), 3);
        let vectors = provider
            .embed_batch(&["first input".into(), "second input".into()])
            .await
            .unwrap();
        assert_eq!(vectors, vec![vec![1.0; 3], vec![2.0; 3]]);
        let observed = observations.lock().await.last().unwrap().clone();
        assert_eq!(observed.2["model"], "native-custom-vector-model");
        assert_eq!(
            observed.1.as_deref(),
            key_required.then_some("Bearer native-stored-embedding-key")
        );
        if protocol == "ollama" {
            assert_eq!(observed.0, "/custom/api/embed");
            assert_eq!(observed.2["truncate"], false);
        } else {
            assert_eq!(observed.0, "/custom/v1/embeddings");
        }
        let mut edited = repo.get(id).await.unwrap().unwrap();
        edited
            .embedding_models
            .0
            .get_mut("native-custom-vector-model")
            .unwrap()
            .cache_max_entries = 0;
        repo.upsert(edited).await.unwrap();
        let uncached = resolve(storage.path().into()).await.unwrap().unwrap();
        assert_eq!(
            uncached.space_id(),
            provider.space_id(),
            "cache edits preserve vector compatibility"
        );
        let mut new_revision = repo.get(id).await.unwrap().unwrap();
        new_revision
            .embedding_models
            .0
            .get_mut("native-custom-vector-model")
            .unwrap()
            .vector_revision = std::num::NonZeroU32::new(2).unwrap();
        repo.upsert(new_revision).await.unwrap();
        let changed = resolve(storage.path().into()).await.unwrap().unwrap();
        assert_ne!(
            changed.space_id(),
            provider.space_id(),
            "explicit model revision selects another vector space"
        );
        let before_disabled = observations.lock().await.len();
        uncached.embed("first input").await.unwrap();
        uncached.embed("first input").await.unwrap();
        assert_eq!(observations.lock().await.len(), before_disabled + 2);
        provider.embed("first input").await.unwrap();
        assert_eq!(
            observations.lock().await.len(),
            before_disabled + 2,
            "old captured adapter retains its exact model/cache contract"
        );
        // No approximate token-to-byte cap may discard eligible input. The
        // actual configured adapter/cache must send every UTF-8 byte unchanged.
        let before = observations.lock().await.len();
        let long = "Ü λ🦀".repeat(5000);
        assert!(long.len() > 32_000);
        assert_eq!(provider.embed(&long).await.unwrap(), vec![1.0; 3]);
        let observed = observations.lock().await.last().unwrap().clone();
        assert_eq!(observed.2["input"], serde_json::json!([long]));
        assert_eq!(observations.lock().await.len(), before + 1);
        assert_eq!(
            provider
                .embed_batch(&[long.clone(), "another full input".into()])
                .await
                .unwrap(),
            vec![vec![1.0; 3], vec![1.0; 3]],
            "cached hit and actual miss retain source order"
        );
        let before_rejection = observations.lock().await.len();
        let rejected = format!("model-limit-fixture:{long}");
        assert!(matches!(
            provider.embed(&rejected).await,
            Err(brassclaw_memory::EmbeddingError::ProviderUnavailable { .. })
        ));
        assert_eq!(
            observations.lock().await.len(),
            before_rejection + 1,
            "actual rejection is not replayed"
        );
        assert_eq!(
            observations.lock().await.last().unwrap().2["input"],
            serde_json::json!([rejected])
        );
        // Consume configured transport controls through this exact persisted
        // provider. Existing captured adapters retain their prior deadline/body
        // contract; fresh resolutions use the new values without retrying.
        let mut bounded = repo.get(id).await.unwrap().unwrap();
        bounded.request_policy.timeout_secs = 1;
        bounded.request_policy.max_response_bytes = Some(128);
        repo.upsert(bounded).await.unwrap();
        let bounded = resolve(storage.path().into()).await.unwrap().unwrap();
        assert_eq!(
            bounded.space_id(),
            changed.space_id(),
            "transport edits do not alter vector compatibility"
        );
        let before_capacity = observations.lock().await.len();
        let capacity = bounded
            .embed("capacity-fixture:private-input")
            .await
            .unwrap_err();
        assert!(
            capacity.to_string().contains("byte capacity (128)"),
            "{protocol}: {capacity}"
        );
        assert!(!capacity.to_string().contains("private-input"));
        assert_eq!(
            observations.lock().await.len(),
            before_capacity + 1,
            "capacity failure never retries"
        );
        assert_eq!(
            uncached
                .embed("capacity-fixture:private-input")
                .await
                .unwrap(),
            vec![1.0; 3],
            "old adapter keeps its unrestricted body contract"
        );
        let before_delay = observations.lock().await.len();
        let deadline = bounded
            .embed("delay-fixture:private-input")
            .await
            .unwrap_err();
        assert!(
            deadline.to_string().contains("timed out"),
            "{protocol}: {deadline}"
        );
        assert_eq!(
            observations.lock().await.len(),
            before_delay + 1,
            "timeout never retries"
        );
        assert_eq!(
            uncached.embed("delay-fixture:private-input").await.unwrap(),
            vec![1.0; 3],
            "old adapter keeps its longer deadline"
        );
        let before_redirect = observations.lock().await.len();
        let redirect = bounded
            .embed("redirect-fixture:private-input")
            .await
            .unwrap_err();
        assert!(
            redirect.to_string().contains("HTTP 307"),
            "{protocol}: {redirect}"
        );
        assert_eq!(
            observations.lock().await.len(),
            before_redirect + 1,
            "redirect cannot forward credentials or replay the request"
        );
        let mut larger = repo.get(id).await.unwrap().unwrap();
        larger.request_policy.max_response_bytes = Some(10_000);
        repo.upsert(larger).await.unwrap();
        assert_eq!(
            resolve(storage.path().into())
                .await
                .unwrap()
                .unwrap()
                .embed("capacity-fixture:private-input")
                .await
                .unwrap(),
            vec![1.0; 3]
        );
        // Actual service failures stay errors, without leaking echoed text,
        // credentials, URLs or issuing another model request.
        for (marker, expected) in [
            ("auth-error-fixture:", "authentication failed"),
            ("rate-error-fixture:", "rate limited"),
            ("service-error-fixture:", "HTTP 500"),
            ("malformed-response-fixture:", "could not be decoded"),
        ] {
            let sensitive_input = format!("{marker}native-private-input");
            let before_error = observations.lock().await.len();
            let error = provider.embed(&sensitive_input).await.unwrap_err();
            let reported = format!("{error}; {error:?}");
            assert!(reported.contains(expected), "{protocol}: {reported}");
            let endpoint_address = address.to_string();
            for secret in [
                sensitive_input.as_str(),
                "native-sensitive-service-payload",
                "native-stored-embedding-key",
                endpoint_address.as_str(),
            ] {
                assert!(!reported.contains(secret), "{protocol}: {reported}");
            }
            assert_eq!(observations.lock().await.len(), before_error + 1);
        }
    }
    // A real refused connection must report its transport category without
    // exposing the selected endpoint through reqwest's formatted error.
    let closed_listener = native_http::bind_loopback().await.unwrap();
    let closed_address = closed_listener.local_addr().unwrap();
    drop(closed_listener);
    let mut disconnected = repo.get("native-optional").await.unwrap().unwrap();
    disconnected.default_base_url = Some(format!("http://{closed_address}/private-endpoint/v1"));
    repo.upsert(disconnected).await.unwrap();
    let disconnected = resolve(storage.path().into()).await.unwrap().unwrap();
    let error = disconnected
        .embed("native-private-input")
        .await
        .unwrap_err();
    let reported = format!("{error}; {error:?}");
    assert!(
        reported.contains("embedding connection failed"),
        "{reported}"
    );
    assert!(!reported.contains("private-endpoint"), "{reported}");
    assert!(
        !reported.contains(&closed_address.to_string()),
        "{reported}"
    );
    assert!(!reported.contains("native-private-input"), "{reported}");
    assert!(
        crate::retention_sweep::run_backfill_embeddings(
            Arc::clone(&rig.pool),
            data_owner,
            0,
            storage.path(),
            secret_owner
        )
        .await
        .is_err()
    );
    assert!(repo.delete("native-optional").await.unwrap());
    assert!(
        resolve(missing_storage.path().into())
            .await
            .unwrap()
            .is_none()
    );
    let mut unsupported = repo.get("native-openai").await.unwrap().unwrap();
    unsupported.protocol = brassclaw_llm::registry::ProviderProtocol::Anthropic;
    repo.upsert(unsupported).await.unwrap();
    save_config_changes(
        pool,
        data_owner,
        &[("embedding.provider_id", Some("native-openai"))],
        ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let before = observations.lock().await.len();
    assert!(resolve(storage.path().into()).await.is_err());
    assert_eq!(observations.lock().await.len(), before);
    save_config_changes(
        pool,
        data_owner,
        &[("embedding.provider_id", Some("absent-provider"))],
        ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    assert!(resolve(storage.path().into()).await.is_err());
    let client = pool.get().await.unwrap();
    client
        .batch_execute(
            "ALTER TABLE brassclaw_llm_providers RENAME TO brassclaw_llm_providers_unavailable",
        )
        .await
        .unwrap();
    let unavailable = resolve(storage.path().into()).await;
    client
        .batch_execute(
            "ALTER TABLE brassclaw_llm_providers_unavailable RENAME TO brassclaw_llm_providers",
        )
        .await
        .unwrap();
    assert!(unavailable.is_err());
    stop.send(()).unwrap();
    server.await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_secret_transaction_fences_absent_row_wrong_key_writer() {
    use brassclaw_host_api::{ResourceScope, SecretHandle};
    use brassclaw_secrets::SecretStore;
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let scope = ResourceScope::system();
    let handle = SecretHandle::new("native-transaction-key").unwrap();
    let store = brassclaw_secrets::PgSecretStore::new(
        rig.pool.as_ref().clone(),
        secrecy::SecretString::from("native-transaction-retained-master-key"),
        "native-transaction-owner",
    )
    .unwrap();
    let wrong = Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            secrecy::SecretString::from("wrong-native-transaction-retained-master-key"),
            "native-transaction-owner",
        )
        .unwrap(),
    );
    let mut connection = rig.pool.get().await.unwrap();
    let transaction = connection.transaction().await.unwrap();
    store
        .put_in_transaction(
            &transaction,
            scope.clone(),
            handle.clone(),
            secrecy::SecretString::from("retained-transaction-material"),
        )
        .await
        .unwrap();
    let wrong_scope = scope.clone();
    let wrong_handle = handle.clone();
    let competing = tokio::spawn(async move {
        wrong
            .put(
                wrong_scope,
                wrong_handle,
                secrecy::SecretString::from("must-never-replace-material"),
            )
            .await
    });
    let observer = rig.pool.get().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let blocked: bool = observer.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE wait_event='advisory' AND query LIKE '%pg_advisory_xact_lock%')", &[]).await.unwrap().get(0);
            if blocked { break; }
            assert!(!competing.is_finished(), "competing writer must wait before authentication");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    transaction.commit().await.unwrap();
    assert!(competing.await.unwrap().is_err());
    let lease = store.lease_once(&scope, &handle).await.unwrap();
    assert_eq!(
        store
            .consume(&scope, lease.id)
            .await
            .unwrap()
            .expose_secret(),
        "retained-transaction-material"
    );
    // An uncommitted setup write cannot survive rollback.
    let transaction = connection.transaction().await.unwrap();
    store
        .put_in_transaction(
            &transaction,
            scope.clone(),
            handle.clone(),
            secrecy::SecretString::from("rolled-back-material"),
        )
        .await
        .unwrap();
    transaction.rollback().await.unwrap();
    let lease = store.lease_once(&scope, &handle).await.unwrap();
    assert_eq!(
        store
            .consume(&scope, lease.id)
            .await
            .unwrap()
            .expose_secret(),
        "retained-transaction-material"
    );
}

/// Actual DB/WebUI role publications and HTTP provider chains. This qualifies
/// response-cache controls, not the model's answer quality or whole-task routing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_live_response_cache_policy_updates_selected_roles() {
    use axum::{Json, Router, extract::State, routing::post};
    use brassclaw_llm::{CompletionRequest, LlmProvider, ResponseCachePolicy};
    use brassclaw_product_workflow::{UpsertLlmProviderRequest, WebUiAuthenticatedCaller};
    struct EndpointState {
        calls: std::sync::atomic::AtomicUsize,
    }
    async fn endpoint(
        State(state): State<Arc<EndpointState>>,
        Json(body): Json<serde_json::Value>,
    ) -> Json<serde_json::Value> {
        state
            .calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Json(
            serde_json::json!({"id":"native-policy-answer","object":"chat.completion","created":1,"model":body["model"],"choices":[{"index":0,"message":{"role":"assistant","content":"policy answer"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":2,"total_tokens":3}}),
        )
    }
    let state = Arc::new(EndpointState {
        calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let listener = native_http::bind_loopback().await.unwrap();
    let address = listener.local_addr().unwrap();
    let shutdown = tokio_util::sync::CancellationToken::new();
    let server_stop = shutdown.clone();
    let app = Router::new()
        .route("/v1/chat/completions", post(endpoint))
        .with_state(state.clone());
    let server = tokio::spawn(async move {
        native_http::serve(listener, app, server_stop.cancelled_owned())
            .await
            .unwrap();
    });
    let home = tempfile::tempdir().unwrap();
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db_guard = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let tenant = "native-cache-policy-tenant";
    let repo = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), tenant);
    let initial = ResponseCachePolicy {
        enabled: true,
        max_entries: 2,
        ttl_secs: 60,
    };
    let definition=serde_json::from_value(serde_json::json!({"id":"native-cache-policy","aliases":[],"protocol":"open_ai_completions","default_base_url":format!("http://{address}/v1"),"base_url_required":true,"api_key_required":false,"model_env":"UNUSED_NATIVE_CACHE_POLICY_MODEL","default_model":"native-cache-model","description":"Native cache policy fixture","context_window_tokens":131072,"response_cache":initial})).unwrap();
    repo.upsert(definition).await.unwrap();
    crate::db_config::save_config_changes(
        &rig.pool,
        tenant,
        &[
            ("llm.default.provider_id", Some("native-cache-policy")),
            ("llm.sempai.provider_id", Some("native-cache-policy")),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let input = RebornRuntimeInput::from_services(
        rig.build_input("native-cache-owner", home.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )
    .with_identity(RebornRuntimeIdentity {
        tenant_id: tenant.into(),
        agent_id: "native-cache-agent".into(),
        source_binding_id: "native-cache-source".into(),
        reply_target_binding_id: "native-cache-reply".into(),
    });
    let runtime = Arc::new(Box::pin(build_reborn_runtime(input)).await.unwrap());
    let bundle = crate::build_webui_services(runtime.clone(), None)
        .await
        .unwrap();
    let primary = runtime
        .llm_reload
        .as_ref()
        .unwrap()
        .reload_handle
        .primary_provider();
    let sempai = runtime.sempai_swappable().unwrap();
    let retained_primary = primary.retained_provider().expect("named primary snapshot");
    let retained_sempai = sempai.retained_selection();
    let request = |text: &str| CompletionRequest {
        messages: vec![brassclaw_llm::ChatMessage::user(text)],
        model: None,
        max_tokens: Some(16),
        temperature: None,
        stop_sequences: None,
        metadata: Default::default(),
    };
    assert_eq!(
        primary
            .complete(request("same request"))
            .await
            .unwrap()
            .input_tokens,
        1
    );
    assert_eq!(
        primary
            .complete(request("same request"))
            .await
            .unwrap()
            .input_tokens,
        0
    );
    assert_eq!(
        sempai
            .complete(request("same request"))
            .await
            .unwrap()
            .input_tokens,
        1
    );
    assert_eq!(
        sempai
            .complete(request("same request"))
            .await
            .unwrap()
            .input_tokens,
        0
    );
    assert_eq!(
        state.calls.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "role caches are independent"
    );
    let caller = WebUiAuthenticatedCaller::new(
        brassclaw_host_api::TenantId::new(tenant).unwrap(),
        brassclaw_host_api::UserId::new("native-cache-owner").unwrap(),
        None,
        None,
    );
    let publish = |policy: ResponseCachePolicy| {
        let api = bundle.api.clone();
        let caller = caller.clone();
        let repo = repo.clone();
        async move {
            let before = repo
                .get_revision("native-cache-policy", false)
                .await
                .unwrap()
                .unwrap();
            let request:UpsertLlmProviderRequest=serde_json::from_value(serde_json::json!({"id":"native-cache-policy","adapter":"open_ai_completions","expected_revision":before.revision,"response_cache":policy})).unwrap();
            let snapshot = api.upsert_llm_provider(caller, request).await?;
            let selected = snapshot
                .providers
                .iter()
                .find(|row| row.id == "native-cache-policy")
                .unwrap();
            assert_eq!(selected.response_cache, policy);
            assert_eq!(selected.revision, before.revision + 1);
            Ok::<_, brassclaw_product_workflow::RebornServicesError>(snapshot)
        }
    };
    publish(ResponseCachePolicy {
        enabled: true,
        max_entries: 1,
        ttl_secs: 60,
    })
    .await
    .unwrap();
    assert!(!Arc::ptr_eq(
        &retained_primary.provider,
        &primary
            .retained_provider()
            .expect("named primary snapshot")
            .provider
    ));
    assert!(!Arc::ptr_eq(
        &retained_sempai.provider,
        &sempai.retained_selection().provider
    ));
    for text in ["capacity first", "capacity second", "capacity first"] {
        primary.complete(request(text)).await.unwrap();
    }
    assert_eq!(
        state.calls.load(std::sync::atomic::Ordering::SeqCst),
        5,
        "live capacity evicts LRU entries"
    );
    publish(ResponseCachePolicy {
        enabled: true,
        max_entries: 2,
        ttl_secs: 0,
    })
    .await
    .unwrap();
    for _ in 0..2 {
        primary.complete(request("zero lifetime")).await.unwrap();
        sempai.complete(request("zero lifetime")).await.unwrap();
    }
    assert_eq!(
        state.calls.load(std::sync::atomic::Ordering::SeqCst),
        9,
        "zero lifetime disables both selected role caches"
    );
    assert_eq!(
        retained_primary
            .provider
            .complete(request("same request"))
            .await
            .unwrap()
            .input_tokens,
        0
    );
    assert_eq!(
        retained_sempai
            .provider
            .complete(request("same request"))
            .await
            .unwrap()
            .input_tokens,
        0
    );
    assert_eq!(
        state.calls.load(std::sync::atomic::Ordering::SeqCst),
        9,
        "old retained selections remain intact"
    );
    publish(ResponseCachePolicy {
        enabled: false,
        max_entries: 2,
        ttl_secs: 60,
    })
    .await
    .unwrap();
    for _ in 0..2 {
        primary.complete(request("disabled cache")).await.unwrap();
    }
    assert_eq!(state.calls.load(std::sync::atomic::Ordering::SeqCst), 11);
    publish(ResponseCachePolicy {
        enabled: true,
        max_entries: 0,
        ttl_secs: 60,
    })
    .await
    .unwrap();
    for _ in 0..2 {
        primary.complete(request("zero capacity")).await.unwrap();
    }
    assert_eq!(state.calls.load(std::sync::atomic::Ordering::SeqCst), 13);
    publish(ResponseCachePolicy {
        enabled: true,
        max_entries: 2,
        ttl_secs: 1,
    })
    .await
    .unwrap();
    primary.complete(request("expiry request")).await.unwrap();
    primary.complete(request("expiry request")).await.unwrap();
    assert_eq!(state.calls.load(std::sync::atomic::Ordering::SeqCst), 14);
    tokio::time::sleep(Duration::from_millis(1100)).await;
    primary.complete(request("expiry request")).await.unwrap();
    assert_eq!(
        state.calls.load(std::sync::atomic::Ordering::SeqCst),
        15,
        "live lifetime expires after completion"
    );
    let before = repo
        .get_revision("native-cache-policy", false)
        .await
        .unwrap()
        .unwrap();
    assert!(
        publish(ResponseCachePolicy {
            enabled: true,
            max_entries: u64::MAX,
            ttl_secs: 1
        })
        .await
        .is_err()
    );
    assert_eq!(
        repo.get_revision("native-cache-policy", false)
            .await
            .unwrap()
            .unwrap()
            .revision,
        before.revision
    );
    drop(bundle);
    Arc::try_unwrap(runtime)
        .unwrap_or_else(|_| panic!("runtime owner retained after provider policy checks"))
        .shutdown()
        .await
        .unwrap();
    // Startup restores the same policy from DB without resetting it from defaults.
    let input = RebornRuntimeInput::from_services(
        rig.build_input("native-cache-owner", home.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )
    .with_identity(RebornRuntimeIdentity {
        tenant_id: tenant.into(),
        agent_id: "native-cache-agent".into(),
        source_binding_id: "native-cache-source".into(),
        reply_target_binding_id: "native-cache-reply".into(),
    });
    let restarted = build_reborn_runtime(input).await.unwrap();
    let backend = restarted
        .llm_reload
        .as_ref()
        .unwrap()
        .reload_handle
        .primary_provider();
    backend.complete(request("restart request")).await.unwrap();
    backend.complete(request("restart request")).await.unwrap();
    assert_eq!(state.calls.load(std::sync::atomic::Ordering::SeqCst), 16);
    restarted.shutdown().await.unwrap();
    shutdown.cancel();
    server.await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_provider_database_initialization_preserves_operator_inventory() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let repo = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), "fresh-provider-owner");
    let (first, second) = tokio::join!(repo.initialize(), repo.initialize());
    assert_eq!(
        usize::from(first.unwrap()) + usize::from(second.unwrap()),
        1
    );
    let definitions = repo.load_all().await.unwrap();
    assert_eq!(definitions.len(), 27);
    assert!(
        definitions.iter().all(
            |(definition, builtin)| *builtin && definition.response_cache == Default::default()
        )
    );
    let original = repo.get_revision("openai", false).await.unwrap().unwrap();
    let mut edited = original.definition;
    edited.protocol = brassclaw_llm::ProviderProtocol::Ollama;
    edited.default_base_url = Some("http://127.0.0.1:11434".into());
    edited.default_model = "operator-model".into();
    edited.api_key_required = false;
    edited.setup = None;
    edited.aliases = vec!["operator-alias".into()];
    edited.response_cache = brassclaw_llm::ResponseCachePolicy {
        enabled: true,
        max_entries: 3,
        ttl_secs: 7,
    };
    let saved = repo
        .replace_revision(edited, Some(original.revision))
        .await
        .unwrap();
    assert!(!repo.initialize().await.unwrap());
    let restored = repo.get_revision("openai", false).await.unwrap().unwrap();
    assert_eq!(restored.revision, saved.revision);
    assert_eq!(
        serde_json::to_value(restored.definition).unwrap(),
        serde_json::to_value(saved.definition).unwrap()
    );
    let client = rig.pool.get().await.unwrap();
    client.execute("UPDATE brassclaw_llm_providers SET deleted_at=now() WHERE tenant_id='fresh-provider-owner'",&[]).await.unwrap();
    assert!(!repo.initialize().await.unwrap());
    assert!(repo.load_all().await.unwrap().is_empty());
    let retained = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), "retained-provider-owner");
    let mut custom = repo
        .get_revision("openai", true)
        .await
        .unwrap()
        .unwrap()
        .definition;
    custom.id = "operator-provider".into();
    custom.aliases = vec!["openai".into()];
    retained.upsert(custom).await.unwrap();
    assert!(!retained.initialize().await.unwrap());
    assert_eq!(
        retained.load_all().await.unwrap().len(),
        1,
        "initialization cannot inject conflicting aliases into an existing inventory"
    );
    assert!(retained.delete("operator-provider").await.unwrap());
    assert!(!retained.initialize().await.unwrap());
    assert!(retained.load_all().await.unwrap().is_empty());
    for (owner, mode) in [
        ("fresh-provider-owner", "fresh"),
        ("retained-provider-owner", "retained"),
    ] {
        let row=client.query_one("SELECT data_revision,seed_checksum,mode FROM brassclaw_llm_provider_initializations WHERE tenant_id=$1",&[&owner]).await.unwrap();
        assert_eq!(row.get::<_, i64>(0), 1);
        assert_eq!(
            row.get::<_, String>(1),
            "60ac2058daf8b8024b8f65e7c1b4b7b6e66e4faa34c43f6f582cf6b7c96e0391"
        );
        assert_eq!(row.get::<_, String>(2), mode);
    }
    client.batch_execute("CREATE FUNCTION reject_provider_initialization_history() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.tenant_id='rejected-provider-owner' THEN RAISE EXCEPTION 'deliberate initialization rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_provider_initialization_history BEFORE INSERT ON brassclaw_llm_provider_revisions FOR EACH ROW EXECUTE FUNCTION reject_provider_initialization_history();").await.unwrap();
    let rejected = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), "rejected-provider-owner");
    assert!(rejected.initialize().await.is_err());
    assert!(rejected.load_all().await.unwrap().is_empty());
    assert!(!client.query_one("SELECT EXISTS(SELECT 1 FROM brassclaw_llm_provider_initializations WHERE tenant_id='rejected-provider-owner')",&[]).await.unwrap().get::<_,bool>(0));
    client.batch_execute("DROP TRIGGER reject_provider_initialization_history ON brassclaw_llm_provider_revisions; DROP FUNCTION reject_provider_initialization_history();").await.unwrap();
    assert!(rejected.initialize().await.unwrap());
    for statement in [
        "UPDATE brassclaw_llm_provider_initializations SET mode='retained' WHERE tenant_id='fresh-provider-owner'",
        "DELETE FROM brassclaw_llm_provider_initializations WHERE tenant_id='fresh-provider-owner'",
        "TRUNCATE brassclaw_llm_provider_initializations",
    ] {
        let error = client.batch_execute(statement).await.unwrap_err();
        assert_eq!(error.as_db_error().unwrap().code().code(), "42501");
    }
    // The function's schema-bound search_path defeats session-local shadows.
    client.batch_execute("CREATE TEMP TABLE brassclaw_llm_providers(tenant_id text,id text,definition jsonb,is_builtin bool); CREATE TEMP TABLE brassclaw_llm_provider_initializations(tenant_id text,data_revision bigint,seed_checksum text,mode text);").await.unwrap();
    assert!(client.query_one("SELECT public.brassclaw_initialize_provider_catalogue('schema-bound-provider-owner')",&[]).await.unwrap().get::<_,bool>(0));
    assert_eq!(client.query_one("SELECT count(*) FROM public.brassclaw_llm_providers WHERE tenant_id='schema-bound-provider-owner'",&[]).await.unwrap().get::<_,i64>(0),27);
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM pg_temp.brassclaw_llm_providers", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM pg_temp.brassclaw_llm_provider_initializations",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

#[cfg(feature = "migrate-from-libsql")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_legacy_settings_import_is_atomic_and_preserves_newer_values() {
    use sha2::{Digest, Sha256};
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let directory = tempfile::tempdir().unwrap();
    let home = brassclaw_reborn_config::RebornHome::resolve_from_env_parts(
        Some(directory.path().as_os_str().to_os_string()),
        None,
        None,
    )
    .unwrap();
    let tenant = "native-settings-import";
    let source = home.path().join("config.toml");
    let bytes = b"[runner]\nheartbeat_interval_secs = 13\npoll_interval_ms = 50\n";
    std::fs::write(&source, bytes).unwrap();
    let backup = home.path().join(format!(
        "config.toml.migrated.{}",
        hex::encode(Sha256::digest(bytes))
    ));
    let old_backup = home.path().join("config.toml.migrated");
    std::fs::write(&old_backup, b"retained old backup").unwrap();
    let run = |dry| crate::migration::run_migration(&rig.pool, &home, tenant, dry);
    assert!(run(true).await.unwrap().config_migrated);
    assert!(!backup.exists());
    let observer = rig.pool.get().await.unwrap();
    let count = observer
        .query_one(
            "SELECT count(*) FROM brassclaw_config WHERE tenant_id=$1",
            &[&tenant],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(count, 0);
    observer.batch_execute("CREATE FUNCTION reject_import_config() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.tenant_id='native-settings-import' AND NEW.key='runner.poll_interval_ms' THEN RAISE EXCEPTION 'deliberate settings import rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_import_config BEFORE INSERT ON brassclaw_config FOR EACH ROW EXECUTE FUNCTION reject_import_config();").await.unwrap();
    assert!(run(false).await.is_err());
    assert_eq!(
        observer
            .query_one(
                "SELECT count(*) FROM brassclaw_config WHERE tenant_id=$1",
                &[&tenant]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0,
        "one rejected setting must roll back the entire source"
    );
    assert_eq!(std::fs::read(&source).unwrap(), bytes);
    assert_eq!(std::fs::read(&backup).unwrap(), bytes);
    assert_eq!(std::fs::read(&old_backup).unwrap(), b"retained old backup");
    observer.batch_execute("DROP TRIGGER reject_import_config ON brassclaw_config; DROP FUNCTION reject_import_config();").await.unwrap();
    assert!(run(false).await.unwrap().config_migrated);
    let stamp = observer.query_one("SELECT updated_at::text FROM brassclaw_config WHERE tenant_id=$1 AND key='runner.heartbeat_interval_secs'", &[&tenant]).await.unwrap().get::<_, String>(0);
    assert!(run(false).await.unwrap().config_migrated);
    assert_eq!(observer.query_one("SELECT updated_at::text FROM brassclaw_config WHERE tenant_id=$1 AND key='runner.heartbeat_interval_secs'", &[&tenant]).await.unwrap().get::<_, String>(0), stamp, "matching imports must not reset operator history");
    observer.execute("UPDATE brassclaw_config SET value='99' WHERE tenant_id=$1 AND key='runner.heartbeat_interval_secs'", &[&tenant]).await.unwrap();
    assert!(run(true).await.is_err());
    assert!(run(false).await.is_err());
    assert_eq!(observer.query_one("SELECT value FROM brassclaw_config WHERE tenant_id=$1 AND key='runner.heartbeat_interval_secs'", &[&tenant]).await.unwrap().get::<_, String>(0), "99");
    assert_eq!(std::fs::read(&source).unwrap(), bytes);
    drop(observer);

    let directory = tempfile::tempdir().unwrap();
    let home = brassclaw_reborn_config::RebornHome::resolve_from_env_parts(
        Some(directory.path().as_os_str().to_os_string()),
        None,
        None,
    )
    .unwrap();
    let tenant = "native-sempai-import";
    let source = home.path().join("sempai_provider.json");
    let bytes = serde_json::to_vec(&serde_json::json!({"provider_id":"selected-import-provider","model":"selected-import-model","api_key_env":"LEGACY_SEMPAI_KEY","base_url":"http://127.0.0.1:8000/v1"})).unwrap();
    std::fs::write(&source, &bytes).unwrap();
    let run = |dry| crate::migration::run_migration(&rig.pool, &home, tenant, dry);
    assert!(run(true).await.unwrap().sempai_migrated);
    let observer = rig.pool.get().await.unwrap();
    observer.batch_execute("CREATE FUNCTION reject_import_sempai() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.tenant_id='native-sempai-import' AND NEW.key='llm.sempai.model' THEN RAISE EXCEPTION 'deliberate Sempai import rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_import_sempai BEFORE INSERT ON brassclaw_config FOR EACH ROW EXECUTE FUNCTION reject_import_sempai();").await.unwrap();
    assert!(run(false).await.is_err());
    assert_eq!(
        observer
            .query_one(
                "SELECT count(*) FROM brassclaw_config WHERE tenant_id=$1",
                &[&tenant]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    observer.batch_execute("DROP TRIGGER reject_import_sempai ON brassclaw_config; DROP FUNCTION reject_import_sempai();").await.unwrap();
    assert!(run(false).await.unwrap().sempai_migrated);
    for (key, expected) in [
        ("llm.sempai.provider_id", "selected-import-provider"),
        ("llm.sempai.model", "selected-import-model"),
        ("llm.sempai.api_key_env", "LEGACY_SEMPAI_KEY"),
        ("llm.sempai.base_url", "http://127.0.0.1:8000/v1"),
    ] {
        assert_eq!(
            observer
                .query_one(
                    "SELECT value FROM brassclaw_config WHERE tenant_id=$1 AND key=$2",
                    &[&tenant, &key]
                )
                .await
                .unwrap()
                .get::<_, String>(0),
            expected
        );
    }
    observer.execute("UPDATE brassclaw_config SET value='newer-model' WHERE tenant_id=$1 AND key='llm.sempai.model'", &[&tenant]).await.unwrap();
    assert!(run(false).await.is_err());
    assert_eq!(
        observer
            .query_one(
                "SELECT value FROM brassclaw_config WHERE tenant_id=$1 AND key='llm.sempai.model'",
                &[&tenant]
            )
            .await
            .unwrap()
            .get::<_, String>(0),
        "newer-model"
    );
    assert_eq!(std::fs::read(&source).unwrap(), bytes);
    drop(observer);
}

#[cfg(feature = "migrate-from-libsql")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_legacy_master_import_preserves_files_versions_and_wrapping() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let directory = tempfile::tempdir().unwrap();
    let home = brassclaw_reborn_config::RebornHome::resolve_from_env_parts(
        Some(directory.path().as_os_str().to_os_string()),
        None,
        None,
    )
    .unwrap();
    let legacy = home.path().join(".reborn-local-dev-secrets-master-key");
    let canonical = home.path().join(".secrets-master-key");
    let key: Vec<u8> = (0..32).collect();
    std::fs::write(&legacy, format!("{}\n", hex::encode(&key))).unwrap();
    let tenant = "native-master-import";
    let run = |dry| crate::migration::run_migration(&rig.pool, &home, tenant, dry);
    assert!(run(true).await.unwrap().secrets_master_migrated);
    assert!(!canonical.exists());
    let observer = rig.pool.get().await.unwrap();
    observer.batch_execute("CREATE FUNCTION reject_import_master() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.tenant_id='native-master-import' THEN RAISE EXCEPTION 'deliberate master import rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_import_master BEFORE INSERT ON brassclaw_secrets_master FOR EACH ROW EXECUTE FUNCTION reject_import_master();").await.unwrap();
    assert!(run(false).await.is_err());
    assert_eq!(
        crate::secrets_master::read_raw_master_key(&legacy)
            .unwrap()
            .unwrap(),
        key
    );
    assert_eq!(
        crate::secrets_master::read_raw_master_key(&canonical)
            .unwrap()
            .unwrap(),
        key,
        "a synchronized canonical key remains after a failed DB write"
    );
    assert_eq!(
        observer
            .query_one(
                "SELECT count(*) FROM brassclaw_secrets_master WHERE tenant_id=$1",
                &[&tenant]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    observer.batch_execute("DROP TRIGGER reject_import_master ON brassclaw_secrets_master; DROP FUNCTION reject_import_master();").await.unwrap();
    assert!(run(false).await.unwrap().secrets_master_migrated);
    observer
        .execute(
            "UPDATE brassclaw_secrets_master SET version=4 WHERE tenant_id=$1",
            &[&tenant],
        )
        .await
        .unwrap();
    assert!(run(false).await.unwrap().secrets_master_migrated);
    assert_eq!(
        observer
            .query_one(
                "SELECT version FROM brassclaw_secrets_master WHERE tenant_id=$1",
                &[&tenant]
            )
            .await
            .unwrap()
            .get::<_, i32>(0),
        4
    );
    let other: Vec<u8> = (32..64).collect();
    std::fs::write(&legacy, format!("{}\n", hex::encode(&other))).unwrap();
    assert!(run(true).await.is_err());
    assert!(run(false).await.is_err());
    assert_eq!(
        crate::secrets_master::read_raw_master_key(&legacy)
            .unwrap()
            .unwrap(),
        other
    );
    assert_eq!(
        crate::secrets_master::read_raw_master_key(&canonical)
            .unwrap()
            .unwrap(),
        key
    );
    let wrapped =
        crate::secrets_master::wrap_master_key_argon2id(&key, "native-master-wrapping-passphrase")
            .unwrap();
    observer.execute("UPDATE brassclaw_secrets_master SET wrapped_key=$2,algorithm='aes256gcm-argon2id' WHERE tenant_id=$1", &[&tenant, &wrapped]).await.unwrap();
    std::fs::write(&legacy, b"stale invalid key source").unwrap();
    assert!(!run(false).await.unwrap().secrets_master_migrated);
    let row = observer
        .query_one(
            "SELECT version,algorithm,wrapped_key FROM brassclaw_secrets_master WHERE tenant_id=$1",
            &[&tenant],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, i32>(0), 4);
    assert_eq!(row.get::<_, String>(1), "aes256gcm-argon2id");
    assert_eq!(row.get::<_, String>(2), wrapped);
    assert_eq!(std::fs::read(&legacy).unwrap(), b"stale invalid key source");
    assert_eq!(
        crate::secrets_master::read_raw_master_key(&canonical)
            .unwrap()
            .unwrap(),
        key
    );
    drop(observer);
}

#[cfg(feature = "migrate-from-libsql")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_legacy_sqlite_import_captures_wal_and_rolls_back_all_tables() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let directory = tempfile::tempdir().unwrap();
    let home = brassclaw_reborn_config::RebornHome::resolve_from_env_parts(
        Some(directory.path().as_os_str().to_os_string()),
        None,
        None,
    )
    .unwrap();
    let source = home.path().join("reborn-local-dev.db");
    let database = libsql::Builder::new_local(&source).build().await.unwrap();
    let writer = database.connect().unwrap();
    writer.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE safety_config(id TEXT,user_id TEXT,category TEXT,pattern TEXT,is_enabled INTEGER,is_default INTEGER,created_at TEXT,updated_at TEXT); CREATE TABLE settings(user_id TEXT,key TEXT,value TEXT,updated_at TEXT); INSERT INTO safety_config VALUES('wal-rule','admin','blocked_paths','/wal-rule',1,0,'2026-10-12T00:00:00Z','2026-10-12T00:00:00Z'); INSERT INTO settings VALUES('admin','provider_tokens:wal-provider','{\"total_input\":1234}','2026-10-12T00:00:00Z');").await.unwrap();
    assert!(source.with_file_name("reborn-local-dev.db-wal").exists());
    let previous = home.path().join("reborn-local-dev.db.migrated");
    std::fs::write(&previous, b"retained previous database backup").unwrap();
    let run = |dry| crate::migration::run_migration(&rig.pool, &home, "default", dry);
    assert!(run(true).await.unwrap().libsql_db_migrated);
    let observer = rig.pool.get().await.unwrap();
    assert_eq!(
        observer
            .query_one(
                "SELECT count(*) FROM brassclaw_safety_config WHERE tenant_id='default'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    observer.batch_execute("CREATE FUNCTION reject_sqlite_settings_import() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.provider_id='wal-provider' THEN RAISE EXCEPTION 'deliberate SQLite import rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_sqlite_settings_import BEFORE INSERT ON brassclaw_token_settings FOR EACH ROW EXECUTE FUNCTION reject_sqlite_settings_import();").await.unwrap();
    assert!(run(false).await.is_err());
    assert_eq!(
        observer
            .query_one(
                "SELECT count(*) FROM brassclaw_safety_config WHERE tenant_id='default'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0,
        "the later table failure must roll back earlier table writes"
    );
    observer.batch_execute("DROP TRIGGER reject_sqlite_settings_import ON brassclaw_token_settings; DROP FUNCTION reject_sqlite_settings_import();").await.unwrap();
    assert!(run(false).await.unwrap().libsql_db_migrated);
    assert_eq!(observer.query_one("SELECT pattern FROM brassclaw_safety_config WHERE tenant_id='default' AND id='wal-rule'", &[]).await.unwrap().get::<_, String>(0), "/wal-rule");
    assert_eq!(observer.query_one("SELECT settings->>'total_input' FROM brassclaw_token_settings WHERE tenant_id='default' AND provider_id='wal-provider'", &[]).await.unwrap().get::<_, String>(0), "1234");
    assert!(source.exists());
    assert_eq!(
        std::fs::read(&previous).unwrap(),
        b"retained previous database backup"
    );
    let backup = std::fs::read_dir(home.path())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("reborn-local-dev.db.migrated.")
        })
        .unwrap()
        .path();
    let restored = libsql::Builder::new_local(&backup)
        .flags(libsql::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .build()
        .await
        .unwrap();
    let reader = restored.connect().unwrap();
    let mut rows = reader
        .query(
            "SELECT pattern FROM safety_config WHERE id='wal-rule'",
            libsql::params![],
        )
        .await
        .unwrap();
    assert_eq!(
        rows.next()
            .await
            .unwrap()
            .unwrap()
            .get::<String>(0)
            .unwrap(),
        "/wal-rule",
        "the standalone backup contains committed WAL data"
    );
    writer.execute_batch("CREATE TABLE root_filesystem_entries(path TEXT,contents BLOB,is_dir INTEGER,content_type TEXT,kind TEXT,indexed TEXT,version INTEGER,created_at TEXT,updated_at TEXT); INSERT INTO root_filesystem_entries VALUES('/legacy-file',X'6869',0,'text/plain',NULL,'{}',3,'2026-10-12T00:00:00Z','2026-10-12T00:00:00Z'); CREATE TABLE root_filesystem_index_specs(prefix TEXT,name TEXT,keys TEXT,kind TEXT); INSERT INTO root_filesystem_index_specs VALUES('/','legacy-index','[]','btree'); CREATE TABLE root_filesystem_events(id INTEGER,path TEXT,payload BLOB,created_at TEXT); INSERT INTO root_filesystem_events VALUES(7,'/legacy-file',X'656e74','2026-10-12T00:00:00Z'); CREATE TABLE capability_permissions(tenant_id TEXT,capability_id TEXT,permission_mode TEXT,updated_at TEXT); INSERT INTO capability_permissions VALUES('default','legacy-capability','deny','2026-10-12T00:00:00Z'); CREATE TABLE hooks_predicate_invocations(key_hash BLOB,scope_hash BLOB,event_id TEXT,recorded_at INTEGER); INSERT INTO hooks_predicate_invocations VALUES(zeroblob(32),zeroblob(32),'legacy-event',0); CREATE TABLE hooks_predicate_values(key_hash BLOB,scope_hash BLOB,event_id TEXT,value TEXT,recorded_at INTEGER); INSERT INTO hooks_predicate_values VALUES(zeroblob(32),zeroblob(32),'legacy-event','12.5',0); CREATE TABLE trigger_records(id TEXT,tenant_id TEXT,creator_user_id TEXT,name TEXT,trigger_kind TEXT,trigger_config TEXT,status TEXT,created_at TEXT); INSERT INTO trigger_records VALUES('legacy-trigger','default','admin','Legacy trigger','cron','{}','active','2026-10-12T00:00:00Z');").await.unwrap();
    let upgraded = run(false).await.unwrap();
    assert_eq!(
        upgraded.paused_legacy_triggers, 1,
        "legacy effects need reconciliation before activation"
    );
    assert_eq!(
        observer
            .query_one(
                "SELECT state FROM brassclaw_triggers WHERE trigger_id='legacy-trigger'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, String>(0),
        "paused"
    );
    assert_eq!(
        observer
            .query_one(
                "SELECT contents,version FROM brassclaw_root_filesystem WHERE path='/legacy-file'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, Vec<u8>>(0),
        b"hi"
    );
    assert_eq!(observer.query_one("SELECT extract(epoch FROM occurred_at)::bigint FROM hooks_predicate_invocations WHERE event_id='legacy-event'", &[]).await.unwrap().get::<_, i64>(0), 0);
    assert_eq!(
        observer
            .query_one(
                "SELECT value::text FROM hooks_predicate_values WHERE event_id='legacy-event'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, String>(0),
        "12.5"
    );
    let next: i64 = observer.query_one("INSERT INTO brassclaw_root_filesystem_events(tenant_id,path,payload) VALUES('default','/new-event',$1) RETURNING seq", &[&&b"new"[..]]).await.unwrap().get(0);
    assert_eq!(next, 8);
    writer.execute_batch("INSERT INTO safety_config VALUES('must-roll-back','admin','blocked_paths','/must-roll-back',1,0,'2026-10-12T00:00:00Z','2026-10-12T00:00:00Z'); CREATE TABLE memory_docs(user_id TEXT,project_id TEXT,id TEXT,doc_type TEXT,title TEXT,content TEXT,source_thread_id BLOB,tags_json TEXT,metadata_json TEXT,created_at TEXT,updated_at TEXT); INSERT INTO memory_docs VALUES('admin','default','bad-optional-metadata','note','invalid','text',X'01','[]','{}','2026-10-12T00:00:00Z','2026-10-12T00:00:00Z');").await.unwrap();
    assert!(
        run(true).await.is_err(),
        "invalid optional values cannot become missing defaults"
    );
    assert!(run(false).await.is_err());
    assert_eq!(
        observer
            .query_one(
                "SELECT count(*) FROM brassclaw_safety_config WHERE id='must-roll-back'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    writer.execute_batch("DROP TABLE memory_docs; UPDATE settings SET value='{\"total_input\":9999}' WHERE key='provider_tokens:wal-provider';").await.unwrap();
    assert!(
        run(false).await.is_err(),
        "a conflicting retained row must not be silently skipped"
    );
    assert_eq!(observer.query_one("SELECT settings->>'total_input' FROM brassclaw_token_settings WHERE provider_id='wal-provider'", &[]).await.unwrap().get::<_,String>(0), "1234");
    assert_eq!(
        observer
            .query_one(
                "SELECT count(*) FROM brassclaw_safety_config WHERE id='must-roll-back'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    drop(observer);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_embedding_model_contract_management_and_upgrade_are_revisioned() {
    use brassclaw_product_workflow::LlmConfigService;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let owner = "embedding-contract-operator";
    let repo = Arc::new(crate::PgProviderRepo::new(rig.pool.as_ref().clone(), owner));
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            secrecy::SecretString::from("native-embedding-contract-retained-master-key"),
            owner,
        )
        .unwrap(),
    ));
    let primary = Arc::new(brassclaw_llm::SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let reload = Arc::new(crate::llm_reload::RebornLlmReloadAdapter::new(
        Arc::new(brassclaw_llm::LlmReloadHandle::new(primary, None)),
        Arc::new(brassclaw_llm::SessionManager::new(Default::default())),
        keys.clone(),
        rig.pool.clone(),
        owner,
    ));
    let service = crate::RebornLlmConfigService::new(keys, rig.pool.clone(), repo.clone(), owner)
        .with_reload_trigger(reload);
    let caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        brassclaw_host_api::TenantId::new(owner).unwrap(),
        brassclaw_host_api::UserId::new("contract-operator").unwrap(),
        None,
        None,
    );
    let models = serde_json::json!({"custom-model":{"dimension":3,"cache_max_entries":0}});
    let saved = service.upsert_provider(caller.clone(), serde_json::from_value(serde_json::json!({
        "id":"edited-vector-provider","adapter":"open_ai_completions","base_url":"http://127.0.0.1:8000/v1",
        "default_model":"custom-model","embedding_models":models
    })).unwrap()).await.unwrap();
    let view = saved
        .providers
        .iter()
        .find(|p| p.id == "edited-vector-provider")
        .unwrap();
    assert_eq!(
        serde_json::to_value(&view.embedding_models).unwrap(),
        models
    );
    let revision = view.revision;
    let saved = service.upsert_provider(caller.clone(), serde_json::from_value(serde_json::json!({
        "id":"edited-vector-provider","expected_revision":revision,"adapter":"open_ai_completions","name":"Renamed provider"
    })).unwrap()).await.unwrap();
    let view = saved
        .providers
        .iter()
        .find(|p| p.id == "edited-vector-provider")
        .unwrap();
    assert_eq!(
        serde_json::to_value(&view.embedding_models).unwrap(),
        models,
        "omitted maps preserve explicit zero"
    );
    let current_revision = view.revision;
    assert!(matches!(service.upsert_provider(caller.clone(), serde_json::from_value(serde_json::json!({
        "id":"edited-vector-provider","expected_revision":revision,"adapter":"open_ai_completions","embedding_models":{}
    })).unwrap()).await, Err(brassclaw_product_workflow::LlmConfigServiceError::Conflict{..})));
    let before = repo
        .get_revision("edited-vector-provider", false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.revision, current_revision);
    let rejected=service.upsert_provider(caller.clone(),serde_json::from_value(serde_json::json!({
        "id":"edited-vector-provider","expected_revision":current_revision,"adapter":"anthropic","embedding_models":models
    })).unwrap()).await;
    assert!(matches!(
        rejected,
        Err(brassclaw_product_workflow::LlmConfigServiceError::InvalidRequest { .. })
    ));
    assert_eq!(
        repo.get_revision("edited-vector-provider", false)
            .await
            .unwrap()
            .unwrap()
            .revision,
        current_revision
    );
    for invalid in [
        r#"{"id":"p","adapter":"ollama","embedding_models":null}"#,
        r#"{"id":"p","adapter":"ollama","embedding_models":{"m":{"dimension":0,"cache_max_entries":0}}}"#,
        r#"{"id":"p","adapter":"ollama","embedding_models":{"m":{"dimension":1,"cache_max_entries":0},"m":{"dimension":2,"cache_max_entries":0}}}"#,
    ] {
        assert!(
            serde_json::from_str::<brassclaw_product_workflow::UpsertLlmProviderRequest>(invalid)
                .is_err()
        );
    }

    let mut client = rig.pool.get().await.unwrap();
    let old:serde_json::Value=client.query_one("SELECT definition FROM brassclaw_llm_provider_revisions WHERE tenant_id=$1 AND provider_id='edited-vector-provider' AND revision=$2",&[&owner,&(revision as i64)]).await.unwrap().get(0);
    assert_eq!(old["embedding_models"], models);
    let upgrade = client.transaction().await.unwrap();
    upgrade.batch_execute("CREATE SCHEMA embedding_contract_upgrade; SET LOCAL search_path TO embedding_contract_upgrade,pg_catalog; CREATE TABLE brassclaw_llm_providers(tenant_id text NOT NULL,id text NOT NULL,definition jsonb NOT NULL,is_builtin boolean NOT NULL DEFAULT false,deleted_at timestamptz,PRIMARY KEY(tenant_id,id)); CREATE TABLE brassclaw_config(tenant_id text NOT NULL,key text NOT NULL,value text NOT NULL,PRIMARY KEY(tenant_id,key));").await.unwrap();
    for (id, explicit, removed) in [
        ("missing", false, false),
        ("override", true, true),
        ("empty", true, false),
    ] {
        let mut def = serde_json::json!({"id":id,"aliases":[format!("alias-{id}")],"protocol":"open_ai_completions","default_model":"chat-model"});
        if explicit {
            def["embedding_models"] = if id == "empty" {
                serde_json::json!({})
            } else {
                models.clone()
            };
        }
        upgrade.execute("INSERT INTO brassclaw_llm_providers(tenant_id,id,definition,is_builtin,deleted_at) VALUES($1,$1,$2,true,CASE WHEN $3 THEN now() END)",&[&id,&def,&removed]).await.unwrap();
        upgrade.execute("INSERT INTO brassclaw_config(tenant_id,key,value) VALUES($1,'embedding.provider_id',$2),($1,'embedding.model','legacy-custom-model')",&[&id,&format!("alias-{id}")]).await.unwrap();
    }
    upgrade
        .batch_execute(include_str!(
            "../../../../brassclaw_pg/migrations/V138__provider_definition_revisions.sql"
        ))
        .await
        .unwrap();
    upgrade
        .batch_execute(include_str!(
            "../../../../brassclaw_pg/migrations/V140__provider_database_initialization.sql"
        ))
        .await
        .unwrap();
    upgrade
        .batch_execute(include_str!(
            "../../../../brassclaw_pg/migrations/V151__embedding_model_contracts.sql"
        ))
        .await
        .unwrap();
    let rows=upgrade.query("SELECT id,definition,revision,deleted_at IS NOT NULL FROM brassclaw_llm_providers ORDER BY id",&[]).await.unwrap();
    for row in rows {
        let id: String = row.get(0);
        let def: serde_json::Value = row.get(1);
        match id.as_str() {
            "missing" => {
                assert_eq!(row.get::<_, i64>(2), 2);
                assert_eq!(
                    def["embedding_models"]["legacy-custom-model"],
                    serde_json::json!({"dimension":1536,"cache_max_entries":10000})
                );
                assert_eq!(
                    def["embedding_models"]["nomic-embed-text"]["dimension"],
                    768
                );
            }
            "override" => {
                assert_eq!(row.get::<_, i64>(2), 1);
                assert_eq!(def["embedding_models"], models);
                assert!(row.get::<_, bool>(3));
            }
            "empty" => {
                assert_eq!(row.get::<_, i64>(2), 1);
                assert_eq!(def["embedding_models"], serde_json::json!({}));
            }
            _ => unreachable!(),
        }
    }
    let original:serde_json::Value=upgrade.query_one("SELECT definition FROM brassclaw_llm_provider_revisions WHERE tenant_id='missing' AND provider_id='missing' AND revision=1",&[]).await.unwrap().get(0);
    assert!(original.get("embedding_models").is_none());
    assert_eq!(upgrade.query_one("SELECT value FROM brassclaw_config WHERE tenant_id='missing' AND key='embedding.model'",&[]).await.unwrap().get::<_,String>(0),"legacy-custom-model");
    for invalid in [
        serde_json::json!({"m":{"dimension":0,"cache_max_entries":0}}),
        serde_json::json!({"m":{"dimension":3,"cache_max_entries":-1}}),
        serde_json::json!({"m":{"dimension":3,"cache_max_entries":0,"extra":true}}),
    ] {
        assert!(
            !upgrade
                .query_one(
                    "SELECT brassclaw_embedding_model_contracts_valid($1)",
                    &[&invalid]
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
        );
    }
    assert!(
        upgrade
            .query_one(
                "SELECT brassclaw_initialize_provider_catalogue('new-contract-owner')",
                &[]
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    let seeded:serde_json::Value=upgrade.query_one("SELECT definition FROM brassclaw_llm_providers WHERE tenant_id='new-contract-owner' AND id='openai'",&[]).await.unwrap().get(0);
    assert_eq!(
        seeded["embedding_models"]["text-embedding-3-small"]["dimension"],
        1536
    );
    assert!(
        !upgrade
            .query_one(
                "SELECT brassclaw_initialize_provider_catalogue('new-contract-owner')",
                &[]
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    upgrade.rollback().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pg_memory_vector_spaces_filter_before_ranking() {
    use brassclaw_memory::{
        EmbeddingSpaceId, FilesystemMemoryDocumentRepository, MemoryChunkWrite,
        MemoryDocumentIndexRepository, MemoryDocumentPath, MemoryDocumentRepository,
        MemorySearchRequest, content_sha256,
    };
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let backend = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    backend.run_migrations().await.unwrap();
    let filesystem: Arc<dyn brassclaw_filesystem::RootFilesystem> = backend;
    let repo = FilesystemMemoryDocumentRepository::new(filesystem);
    let selected = EmbeddingSpaceId::from_digest([1; 32]);
    let other = EmbeddingSpaceId::from_digest([2; 32]);
    let a = MemoryDocumentPath::new_with_agent(
        "space-owner",
        "space-user",
        None,
        None,
        "notes/selected.md",
    )
    .unwrap();
    for (name, text, vector, space) in [
        (
            "notes/selected.md",
            "selected document",
            vec![0.0, 1.0, 0.0],
            Some(selected.clone()),
        ),
        (
            "notes/other.md",
            "other document",
            vec![1.0, 0.0, 0.0],
            Some(other),
        ),
        (
            "notes/legacy.md",
            "legacy document",
            vec![1.0, 0.0, 0.0],
            None,
        ),
    ] {
        let path =
            MemoryDocumentPath::new_with_agent("space-owner", "space-user", None, None, name)
                .unwrap();
        repo.write_document(&path, text.as_bytes()).await.unwrap();
        repo.replace_document_chunks_if_current(
            &path,
            &content_sha256(text),
            &[MemoryChunkWrite {
                content: text.into(),
                embedding: Some(vector),
                embedding_space: space,
                chat_record_id: None,
            }],
        )
        .await
        .unwrap();
    }
    let query = MemorySearchRequest::new("document")
        .unwrap()
        .with_limit(1)
        .with_full_text(false)
        .with_query_embedding(vec![1.0, 0.0, 0.0])
        .with_embedding_space(selected);
    let hits = repo.search_documents(a.scope(), &query).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, a);
    let text = MemorySearchRequest::new("legacy")
        .unwrap()
        .with_vector(false);
    assert_eq!(
        repo.search_documents(a.scope(), &text).await.unwrap()[0]
            .path
            .relative_path(),
        "notes/legacy.md"
    );
    let unknown = MemorySearchRequest::new("document")
        .unwrap()
        .with_full_text(false)
        .with_query_embedding(vec![1.0, 0.0, 0.0]);
    assert!(repo.search_documents(a.scope(), &unknown).await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_live_memory_embedding_uses_registered_tools_and_current_db_selection() {
    use axum::{Json, Router, extract::State, response::IntoResponse, routing::post};
    use brassclaw_authorization::{
        InstanceToolRule, LiveStableToolPolicy, StableToolPolicySnapshot, ToolExecutionRules,
    };
    use brassclaw_host_api::{
        CapabilitySet, ExecutionContext, ExtensionId, MountAlias, MountGrant, MountPermissions,
        MountView, NetworkPolicy, RuntimeKind, TrustClass, VirtualPath,
    };
    use brassclaw_host_runtime::{
        MEMORY_READ_CAPABILITY_ID, MEMORY_SEARCH_CAPABILITY_ID, MEMORY_TREE_CAPABILITY_ID,
        MEMORY_WRITE_CAPABILITY_ID, RuntimeCapabilityOutcome, RuntimeCapabilityRequest,
    };
    use brassclaw_memory::EmbeddingProviderSource;
    use brassclaw_trust::{AuthorityCeiling, EffectiveTrustClass, TrustDecision, TrustProvenance};
    use serde_json::json;
    use std::collections::HashMap;
    type Calls = Arc<tokio::sync::Mutex<Vec<serde_json::Value>>>;
    async fn endpoint(
        State(calls): State<Calls>,
        Json(body): Json<serde_json::Value>,
    ) -> axum::response::Response {
        calls.lock().await.push(body.clone());
        if body["input"][0]
            .as_str()
            .unwrap()
            .starts_with("outage fixture:")
        {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"private-service-payload"})),
            )
                .into_response();
        }
        let vector = if body["model"] == "space-a" {
            vec![1.0, 0.0, 0.0]
        } else {
            vec![0.0, 1.0, 0.0]
        };
        Json(
            json!({"data":body["input"].as_array().unwrap().iter().enumerate().map(|(i,_)|json!({"index":i,"embedding":vector})).collect::<Vec<_>>()}),
        ).into_response()
    }
    let calls: Calls = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let listener = native_http::bind_loopback().await.unwrap();
    let address = listener.local_addr().unwrap();
    let stop = tokio_util::sync::CancellationToken::new();
    let stopped = stop.clone();
    let app = Router::new()
        .route("/v1/embeddings", post(endpoint))
        .with_state(calls.clone());
    let server = tokio::spawn(async move {
        native_http::serve(listener, app, stopped.cancelled_owned())
            .await
            .unwrap();
    });
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let root = tempfile::tempdir().unwrap();
    let owner = "live-memory-owner";
    let repo = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), "default");
    repo.upsert(serde_json::from_value(json!({"id":"memory-provider","protocol":"open_ai_completions","default_base_url":format!("http://{address}/v1"),"api_key_required":false,"model_env":"UNUSED_MEMORY_MODEL","default_model":"space-a","description":"Native memory transport fixture","embedding_models":{"space-a":{"dimension":3,"cache_max_entries":8},"space-b":{"dimension":3,"cache_max_entries":0}}})).unwrap()).await.unwrap();
    crate::db_config::save_config_changes(
        &rig.pool,
        "default",
        &[
            ("embedding.provider_id", Some("memory-provider")),
            ("embedding.model", Some("space-a")),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let runtime = Arc::new(
        Box::pin(build_reborn_runtime(
            RebornRuntimeInput::from_services(
                rig.build_input(owner, root.path())
                    .with_runtime_policy(local_dev_runtime_policy()),
            )
            .with_identity(RebornRuntimeIdentity {
                tenant_id: "default".into(),
                ..RebornRuntimeIdentity::reborn_cli()
            }),
        ))
        .await
        .unwrap(),
    );
    let vm = runtime.global_monty_owner.client().root_identity();
    let source = runtime.services.memory_embedding_source.clone().unwrap();
    let first = source.snapshot().await.unwrap().unwrap();
    assert!(
        Arc::ptr_eq(&first, &source.snapshot().await.unwrap().unwrap()),
        "stable snapshot retains real cache/client"
    );
    let mounts = MountView::new(vec![MountGrant::new(
        MountAlias::new("/memory").unwrap(),
        VirtualPath::new("/memory").unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    let caps = [
        MEMORY_WRITE_CAPABILITY_ID,
        MEMORY_SEARCH_CAPABILITY_ID,
        MEMORY_READ_CAPABILITY_ID,
        MEMORY_TREE_CAPABILITY_ID,
    ];
    let mut tools = HashMap::new();
    let mut mappings = HashMap::new();
    for cap in caps {
        let id = uuid::Uuid::new_v4();
        mappings.insert(CapabilityId::new(cap).unwrap(), id);
        tools.insert(
            id,
            InstanceToolRule {
                enabled: true,
                revision: 1,
                execution: ToolExecutionRules {
                    allowed_effects: vec![EffectKind::ReadFilesystem, EffectKind::WriteFilesystem],
                    mounts: mounts.clone(),
                    network: NetworkPolicy::default(),
                    secrets: vec![],
                    resource_ceiling: None,
                },
            },
        );
    }
    let mut policy_snapshot = StableToolPolicySnapshot {
        revision: 1,
        tools,
        capabilities: mappings,
    };
    let policy = Arc::new(LiveStableToolPolicy::new(policy_snapshot.clone()).unwrap());
    let kernel = runtime.services.monty_kernel.as_ref().unwrap();
    let mut retained = HashMap::new();
    for cap in caps {
        let cap = CapabilityId::new(cap).unwrap();
        let native = kernel.builtin(&cap, None).unwrap();
        native.verify_native_artifact().await.unwrap();
        retained.insert(
            cap.clone(),
            kernel
                .builtin_with_policy(&cap, native.native_identity().unwrap(), policy.clone())
                .unwrap(),
        );
    }
    let invoke = |cap: &str, input| {
        let context = ExecutionContext::local_default(
            UserId::new(owner).unwrap(),
            ExtensionId::new("builtin").unwrap(),
            RuntimeKind::FirstParty,
            TrustClass::FirstParty,
            CapabilitySet::default(),
            mounts.clone(),
        )
        .unwrap();
        RuntimeCapabilityRequest::new(
            context,
            CapabilityId::new(cap).unwrap(),
            ResourceEstimate::default(),
            input,
            TrustDecision {
                effective_trust: EffectiveTrustClass::user_trusted(),
                authority_ceiling: AuthorityCeiling {
                    allowed_effects: vec![EffectKind::ReadFilesystem, EffectKind::WriteFilesystem],
                    max_resource_ceiling: None,
                },
                provenance: TrustProvenance::AdminConfig,
                evaluated_at: chrono::Utc::now(),
            },
        )
    };
    let write = retained[&CapabilityId::new(MEMORY_WRITE_CAPABILITY_ID).unwrap()]
        .invoke(invoke(
            MEMORY_WRITE_CAPABILITY_ID,
            json!({"target":"notes/one.md","content":"violet record","append":false}),
        ))
        .await
        .unwrap();
    assert!(matches!(write, RuntimeCapabilityOutcome::Completed(_)));
    assert_eq!(calls.lock().await.last().unwrap()["model"], "space-a");
    let search = |text: &str| invoke(MEMORY_SEARCH_CAPABILITY_ID, json!({"query":text,"limit":5}));
    let RuntimeCapabilityOutcome::Completed(result) = retained
        [&CapabilityId::new(MEMORY_SEARCH_CAPABILITY_ID).unwrap()]
        .invoke(search("semantic needle"))
        .await
        .unwrap()
    else {
        panic!("actual vector query completion required")
    };
    let result = result.output;
    assert_eq!(
        result["result_count"], 1,
        "query has no lexical overlap; vector branch must be active"
    );
    let before_outage = calls.lock().await.len();
    let RuntimeCapabilityOutcome::Completed(outage) = retained[&CapabilityId::new(MEMORY_WRITE_CAPABILITY_ID).unwrap()]
        .invoke(invoke(MEMORY_WRITE_CAPABILITY_ID, json!({"target":"notes/outage.md","content":"outage fixture: durable document","append":false})))
        .await.unwrap() else { panic!("a committed file must remain a completed write") };
    assert_eq!(outage.output["status"], "written");
    assert_eq!(outage.output["indexing_status"], "incomplete");
    assert!(!outage.output["warnings"].as_array().unwrap().is_empty());
    assert!(
        !outage
            .output
            .to_string()
            .contains("private-service-payload")
    );
    assert_eq!(
        calls.lock().await.len(),
        before_outage + 1,
        "indexing failure never repeats the file or provider effect"
    );
    let RuntimeCapabilityOutcome::Completed(read_outage) = retained
        [&CapabilityId::new(MEMORY_READ_CAPABILITY_ID).unwrap()]
        .invoke(invoke(
            MEMORY_READ_CAPABILITY_ID,
            json!({"path":"notes/outage.md"}),
        ))
        .await
        .unwrap()
    else {
        panic!("committed outage document must remain readable")
    };
    assert_eq!(
        read_outage.output["content"],
        "outage fixture: durable document"
    );
    let RuntimeCapabilityOutcome::Completed(recovered) = retained[&CapabilityId::new(MEMORY_WRITE_CAPABILITY_ID).unwrap()]
        .invoke(invoke(MEMORY_WRITE_CAPABILITY_ID, json!({"target":"notes/recovered.md","content":"independent recovered indexing","append":false}))).await.unwrap() else {
        panic!("independent successful write must complete")
    };
    assert!(
        recovered.output.get("indexing_status").is_none(),
        "indexing warnings remain isolated to their operation"
    );
    let bundle = crate::build_webui_services(runtime.clone(), None)
        .await
        .unwrap();
    let caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        brassclaw_host_api::TenantId::new("default").unwrap(),
        brassclaw_host_api::UserId::new(owner).unwrap(),
        None,
        None,
    );
    let initial_definition = repo.get("memory-provider").await.unwrap().unwrap();
    let cache_edit = |revision, capacity| {
        let mut models = initial_definition.embedding_models.clone();
        models.0.get_mut("space-a").unwrap().cache_max_entries = capacity;
        serde_json::from_value(json!({"id":"memory-provider","expected_revision":revision,
            "adapter":"open_ai_completions","embedding_models":models}))
        .unwrap()
    };
    let before_rejection = repo
        .get_revision("memory-provider", false)
        .await
        .unwrap()
        .unwrap()
        .revision;
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE FUNCTION reject_memory_cache_edit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.provider_id='memory-provider' THEN RAISE EXCEPTION 'deliberate cache publication rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_memory_cache_edit BEFORE INSERT ON brassclaw_llm_provider_revisions FOR EACH ROW EXECUTE FUNCTION reject_memory_cache_edit()").await.unwrap();
    assert!(
        bundle
            .api
            .upsert_llm_provider(caller.clone(), cache_edit(before_rejection, 0))
            .await
            .is_err()
    );
    client.batch_execute("DROP TRIGGER reject_memory_cache_edit ON brassclaw_llm_provider_revisions; DROP FUNCTION reject_memory_cache_edit()").await.unwrap();
    drop(client);
    assert_eq!(
        repo.get_revision("memory-provider", false)
            .await
            .unwrap()
            .unwrap()
            .revision,
        before_rejection
    );
    let before_cached = calls.lock().await.len();
    first.embed("semantic needle").await.unwrap();
    assert_eq!(
        calls.lock().await.len(),
        before_cached,
        "failed persistence does not resize the live cache"
    );
    for capacity in [0, 4] {
        let revision = repo
            .get_revision("memory-provider", false)
            .await
            .unwrap()
            .unwrap()
            .revision;
        bundle
            .api
            .upsert_llm_provider(caller.clone(), cache_edit(revision, capacity))
            .await
            .unwrap();
        let before_probe = calls.lock().await.len();
        // Use the retained provider directly before requesting another source
        // snapshot: the successful WebUI save itself must resize its cache.
        first.embed("semantic needle").await.unwrap();
        first.embed("semantic needle").await.unwrap();
        assert_eq!(
            calls.lock().await.len(),
            before_probe + if capacity == 0 { 2 } else { 1 }
        );
        assert!(
            Arc::ptr_eq(&first, &source.snapshot().await.unwrap().unwrap()),
            "cache-only edits retain the actual client"
        );
    }
    let before_block = calls.lock().await.len();
    let search_tool =
        policy_snapshot.capabilities[&CapabilityId::new(MEMORY_SEARCH_CAPABILITY_ID).unwrap()];
    policy_snapshot.revision = 2;
    for rule in policy_snapshot.tools.values_mut() {
        rule.revision = policy_snapshot.revision;
    }
    policy_snapshot.tools.get_mut(&search_tool).unwrap().enabled = false;
    policy.publish(1, policy_snapshot.clone()).unwrap();
    let RuntimeCapabilityOutcome::Failed(blocked) = retained
        [&CapabilityId::new(MEMORY_SEARCH_CAPABILITY_ID).unwrap()]
        .invoke(search("blocked query"))
        .await
        .unwrap()
    else {
        panic!("live policy must deny the retained Tool")
    };
    assert_eq!(
        blocked.kind,
        brassclaw_host_runtime::RuntimeFailureKind::Authorization
    );
    assert_eq!(
        calls.lock().await.len(),
        before_block,
        "blocked search never contacts the embedding endpoint"
    );
    policy_snapshot.revision = 3;
    for rule in policy_snapshot.tools.values_mut() {
        rule.revision = policy_snapshot.revision;
    }
    policy_snapshot.tools.get_mut(&search_tool).unwrap().enabled = true;
    policy.publish(2, policy_snapshot.clone()).unwrap();
    crate::db_config::save_config_changes(
        &rig.pool,
        "default",
        &[("embedding.model", Some("space-b"))],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let second = source.snapshot().await.unwrap().unwrap();
    assert_ne!(first.space_id(), second.space_id());
    let RuntimeCapabilityOutcome::Completed(result) = retained
        [&CapabilityId::new(MEMORY_SEARCH_CAPABILITY_ID).unwrap()]
        .invoke(search("semantic needle"))
        .await
        .unwrap()
    else {
        panic!("new-space query required")
    };
    let result = result.output;
    assert_eq!(
        result["result_count"], 0,
        "other-space old vectors stay excluded"
    );
    assert_eq!(calls.lock().await.last().unwrap()["model"], "space-b");
    let mut invalid = repo.get("memory-provider").await.unwrap().unwrap();
    invalid.embedding_models.0.remove("space-b");
    repo.upsert(invalid).await.unwrap();
    assert!(
        source.snapshot().await.is_err(),
        "invalid selected metadata never reuses the old provider"
    );
    let invalid_write = retained[&CapabilityId::new(MEMORY_WRITE_CAPABILITY_ID).unwrap()]
        .invoke(invoke(
            MEMORY_WRITE_CAPABILITY_ID,
            json!({"target":"notes/rejected.md","content":"must not persist","append":false}),
        ))
        .await
        .unwrap();
    assert!(
        matches!(invalid_write, RuntimeCapabilityOutcome::Failed(_)),
        "invalid resolution fails before write effects"
    );
    let before = calls.lock().await.len();
    let read = retained[&CapabilityId::new(MEMORY_READ_CAPABILITY_ID).unwrap()]
        .invoke(invoke(
            MEMORY_READ_CAPABILITY_ID,
            json!({"path":"notes/one.md"}),
        ))
        .await
        .unwrap();
    assert!(matches!(read, RuntimeCapabilityOutcome::Completed(_)));
    assert_eq!(
        calls.lock().await.len(),
        before,
        "plain read needs no model resolution"
    );
    bundle
        .api
        .set_active_llm(
            caller.clone(),
            serde_json::from_value(json!({
                "provider_id":"","role":"embedding"
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    assert!(source.snapshot().await.unwrap().is_none());
    let RuntimeCapabilityOutcome::Completed(result) = retained
        [&CapabilityId::new(MEMORY_SEARCH_CAPABILITY_ID).unwrap()]
        .invoke(search("violet"))
        .await
        .unwrap()
    else {
        panic!("disabled embedding retains lexical search")
    };
    let result = result.output;
    assert_eq!(result["result_count"], 1);
    assert_eq!(calls.lock().await.len(), before);
    let RuntimeCapabilityOutcome::Completed(outage_text) = retained
        [&CapabilityId::new(MEMORY_SEARCH_CAPABILITY_ID).unwrap()]
        .invoke(search("outage"))
        .await
        .unwrap()
    else {
        panic!("outage document must retain text search")
    };
    assert_eq!(
        outage_text.output["result_count"], 1,
        "failed embeddings keep complete text-only indexing"
    );
    // Exercise the registered retained Tool and actual PostgreSQL filesystem,
    // after disabling embeddings, including a count crossing the filesystem
    // page size, rather than bypassing dispatch with a helper.
    for index in 0..1_050 {
        let outcome = retained[&CapabilityId::new(MEMORY_WRITE_CAPABILITY_ID).unwrap()]
            .invoke(invoke(
                MEMORY_WRITE_CAPABILITY_ID,
                json!({
                    "target": format!("counts/{index}.md"),
                    "content": format!("nativecountneedle {index}"), "append": false
                }),
            ))
            .await
            .unwrap();
        assert!(matches!(outcome, RuntimeCapabilityOutcome::Completed(_)));
    }
    for (limit, expected) in [(None, 5), (Some(25), 25), (Some(1_050), 1_050)] {
        let mut input = json!({"query":"nativecountneedle"});
        if let Some(limit) = limit {
            input["limit"] = json!(limit);
        }
        let RuntimeCapabilityOutcome::Completed(result) = retained
            [&CapabilityId::new(MEMORY_SEARCH_CAPABILITY_ID).unwrap()]
            .invoke(invoke(MEMORY_SEARCH_CAPABILITY_ID, input))
            .await
            .unwrap()
        else {
            panic!("explicit count search must complete")
        };
        assert_eq!(result.output["result_count"], expected);
        assert_eq!(result.output["results"].as_array().unwrap().len(), expected);
    }
    for limit in [
        json!(0),
        json!(null),
        json!("null"),
        json!(-1),
        json!(1.5),
        json!(u64::from(u32::MAX) + 1),
    ] {
        let outcome = retained[&CapabilityId::new(MEMORY_SEARCH_CAPABILITY_ID).unwrap()]
            .invoke(invoke(
                MEMORY_SEARCH_CAPABILITY_ID,
                json!({"query":"nativecountneedle", "limit":limit}),
            ))
            .await
            .unwrap();
        assert!(
            matches!(outcome, RuntimeCapabilityOutcome::Failed(_)),
            "invalid explicit limit must fail: {limit}"
        );
    }
    assert_eq!(
        calls.lock().await.len(),
        before,
        "text-only count checks must not contact the embedding provider"
    );
    let content = format!("largepayloadneedle ü\n{}", "\"\\".repeat(600_000));
    let RuntimeCapabilityOutcome::Completed(written) = retained
        [&CapabilityId::new(MEMORY_WRITE_CAPABILITY_ID).unwrap()]
        .invoke(invoke(
            MEMORY_WRITE_CAPABILITY_ID,
            json!({
                "target":"large/complete.md","content":content,"append":false
            }),
        ))
        .await
        .unwrap()
    else {
        panic!("large document must be committed once")
    };
    assert_eq!(written.output["content_length"], content.len());
    for (capability, input) in [
        (
            MEMORY_READ_CAPABILITY_ID,
            json!({"path":"large/complete.md"}),
        ),
        (
            MEMORY_SEARCH_CAPABILITY_ID,
            json!({"query":"largepayloadneedle","limit":1}),
        ),
    ] {
        let RuntimeCapabilityOutcome::Completed(result) = retained
            [&CapabilityId::new(capability).unwrap()]
            .invoke(invoke(capability, input))
            .await
            .unwrap()
        else {
            panic!("complete large Memory result required")
        };
        let actual = if capability == MEMORY_READ_CAPABILITY_ID {
            &result.output["content"]
        } else {
            &result.output["results"][0]["content"]
        };
        assert_eq!(actual.as_str(), Some(content.as_str()));
        let bytes = serde_json::to_vec(&result.output).unwrap().len();
        assert!(bytes > 2 * 1024 * 1024);
        assert_eq!(result.usage.output_bytes, bytes as u64);
    }
    // The retained registration still reads current kernel output policy.
    // An advisory small estimate admits the read; measured output must enforce
    // the actual ceiling. Raising that rule changes the next read immediately.
    let read_id =
        policy_snapshot.capabilities[&CapabilityId::new(MEMORY_READ_CAPABILITY_ID).unwrap()];
    for (revision, maximum) in [(4, 8), (5, 4 * 1024 * 1024)] {
        policy_snapshot.revision = revision;
        for rule in policy_snapshot.tools.values_mut() {
            rule.revision = revision;
        }
        policy_snapshot
            .tools
            .get_mut(&read_id)
            .unwrap()
            .execution
            .resource_ceiling = Some(brassclaw_host_api::ResourceCeiling {
            max_usd: None,
            max_input_tokens: None,
            max_output_tokens: None,
            max_wall_clock_ms: None,
            max_output_bytes: Some(maximum),
            sandbox: None,
        });
        policy
            .publish(revision - 1, policy_snapshot.clone())
            .unwrap();
        let mut request = invoke(
            MEMORY_READ_CAPABILITY_ID,
            json!({"path":"large/complete.md"}),
        );
        request.estimate.output_bytes = Some(8);
        let outcome = retained[&CapabilityId::new(MEMORY_READ_CAPABILITY_ID).unwrap()]
            .invoke(request)
            .await
            .unwrap();
        if maximum == 8 {
            let RuntimeCapabilityOutcome::Failed(failure) = outcome else {
                panic!("current kernel ceiling must reject measured output")
            };
            assert_eq!(
                failure.kind,
                brassclaw_host_runtime::RuntimeFailureKind::OutputTooLarge
            );
        } else {
            let RuntimeCapabilityOutcome::Completed(result) = outcome else {
                panic!("larger live policy must permit the full original content")
            };
            assert_eq!(result.output["content"], content);
        }
    }
    assert_eq!(
        calls.lock().await.len(),
        before,
        "complete lexical data and resource checks never call the disabled embedding service"
    );
    // Same retained primitive, real PostgreSQL settings publication and one VM.
    // Omitted depth follows the acknowledged default; explicit larger requests
    // are exact or fail, never silently reduced to the former maximum of ten.
    use brassclaw_host_runtime::HostPrimitiveSettingsSource;
    use brassclaw_product_workflow::MontyVmSettingsStore;
    let settings = runtime.monty_settings_owner.store();
    let original = settings.get("default", "default").await.unwrap();
    let path = format!("tree-depth/{}leaf.md", "level/".repeat(12));
    assert!(matches!(
        retained[&CapabilityId::new(MEMORY_WRITE_CAPABILITY_ID).unwrap()]
            .invoke(invoke(
                MEMORY_WRITE_CAPABILITY_ID,
                json!({"target":path,"content":"tree depth fixture","append":false})
            ))
            .await
            .unwrap(),
        RuntimeCapabilityOutcome::Completed(_)
    ));
    let tree = |depth: Option<u32>| {
        let mut input = json!({"path":"tree-depth"});
        if let Some(depth) = depth {
            input["depth"] = json!(depth);
        }
        invoke(MEMORY_TREE_CAPABILITY_ID, input)
    };
    assert!(matches!(
        retained[&CapabilityId::new(MEMORY_TREE_CAPABILITY_ID).unwrap()]
            .invoke(tree(Some(13)))
            .await
            .unwrap(),
        RuntimeCapabilityOutcome::Failed(_)
    ));
    let mut limits = original.execution_limits;
    limits.default_memory_tree_depth = 13;
    limits.max_memory_tree_depth = 15;
    let update = serde_json::from_value(
        json!({"expected_revision":original.revision,"execution_limits":limits}),
    )
    .unwrap();
    let changed = settings
        .upsert("default", "default", &update)
        .await
        .unwrap();
    let source = runtime.services.primitive_settings.as_ref().unwrap();
    let captured = source.snapshot().await.unwrap();
    assert_eq!(captured.revision, changed.revision);
    assert_eq!(captured.limits.default_memory_tree_depth, 13);
    for depth in [None, Some(13)] {
        let RuntimeCapabilityOutcome::Completed(result) = retained
            [&CapabilityId::new(MEMORY_TREE_CAPABILITY_ID).unwrap()]
            .invoke(tree(depth))
            .await
            .unwrap()
        else {
            panic!("larger exact tree must complete");
        };
        assert!(
            serde_json::to_string(&result.output)
                .unwrap()
                .contains("leaf.md")
        );
        assert_eq!(
            result.usage.output_bytes,
            serde_json::to_vec(&result.output).unwrap().len() as u64
        );
    }
    for input in [
        json!({"depth":null}),
        json!({"depth":"null"}),
        json!({"depth":0}),
        json!({"depth":16}),
        json!({"path":null}),
    ] {
        assert!(matches!(
            retained[&CapabilityId::new(MEMORY_TREE_CAPABILITY_ID).unwrap()]
                .invoke(invoke(MEMORY_TREE_CAPABILITY_ID, input))
                .await
                .unwrap(),
            RuntimeCapabilityOutcome::Failed(_)
        ));
    }
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE FUNCTION reject_tree_policy_fixture() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.execution_limits->'max_memory_tree_depth' IS DISTINCT FROM OLD.execution_limits->'max_memory_tree_depth' THEN RAISE EXCEPTION 'fixture rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_tree_policy_fixture BEFORE UPDATE ON reborn_monty_vm_settings FOR EACH ROW EXECUTE FUNCTION reject_tree_policy_fixture()").await.unwrap();
    limits.max_memory_tree_depth = 20;
    let failed = serde_json::from_value(
        json!({"expected_revision":changed.revision,"execution_limits":limits}),
    )
    .unwrap();
    assert!(
        settings
            .upsert("default", "default", &failed)
            .await
            .is_err()
    );
    assert_eq!(source.snapshot().await.unwrap().revision, captured.revision);
    assert_eq!(source.snapshot().await.unwrap().limits, captured.limits);
    client.batch_execute("DROP TRIGGER reject_tree_policy_fixture ON reborn_monty_vm_settings; DROP FUNCTION reject_tree_policy_fixture()").await.unwrap();
    let restore = serde_json::from_value(
        json!({"expected_revision":changed.revision,"execution_limits":original.execution_limits}),
    )
    .unwrap();
    let restored = settings
        .upsert("default", "default", &restore)
        .await
        .unwrap();
    assert_eq!(source.snapshot().await.unwrap().revision, restored.revision);
    assert!(matches!(
        retained[&CapabilityId::new(MEMORY_TREE_CAPABILITY_ID).unwrap()]
            .invoke(tree(Some(13)))
            .await
            .unwrap(),
        RuntimeCapabilityOutcome::Failed(_)
    ));
    assert_eq!(runtime.global_monty_owner.client().root_identity(), vm);
    drop(bundle);
    shutdown_shared_runtime(runtime).await.unwrap();
    stop.cancel();
    server.await.unwrap();
}

#[path = "../../../../brassclaw_filesystem/tests/support/namespace_contract.rs"]
mod filesystem_namespace_contract;

#[tokio::test]
async fn native_filesystem_namespace_and_cas_are_atomic_across_connections() {
    use brassclaw_filesystem::{
        CasExpectation, Entry, FileType, PostgresRootFilesystem, RootFilesystem,
    };
    use brassclaw_host_api::VirtualPath;
    use std::time::Duration;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let fs = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    fs.run_migrations().await.unwrap();
    filesystem_namespace_contract::assert_namespace_contract(fs.clone()).await;
    // Hold one actual leaf lock. The waiting write must be observable, but an
    // unrelated sibling must complete while it is held; this is not a global
    // filesystem mutex or an unobserved sleep-based concurrency fixture.
    let leaf = VirtualPath::new("/projects/namespace-wait/blocked").unwrap();
    let key = format!("brassclaw.root-filesystem.namespace:{}", leaf.as_str());
    let mut client = rig.pool.get().await.unwrap();
    let held = client.transaction().await.unwrap();
    held.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        &[&key],
    )
    .await
    .unwrap();
    let pending = {
        let fs = fs.clone();
        let leaf = leaf.clone();
        tokio::spawn(async move {
            fs.put(
                &leaf,
                Entry::bytes(b"blocked".to_vec()),
                CasExpectation::Absent,
            )
            .await
        })
    };
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: bool = held.query_one("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND NOT granted AND database=(SELECT oid FROM pg_database WHERE datname=current_database()))", &[]).await.unwrap().get(0);
            if waiting { break; }
            tokio::task::yield_now().await;
        }
    }).await.expect("actual pending namespace lock must be observed");
    let sibling = VirtualPath::new("/projects/namespace-wait/sibling").unwrap();
    tokio::time::timeout(
        Duration::from_secs(5),
        fs.put(
            &sibling,
            Entry::bytes(b"sibling".to_vec()),
            CasExpectation::Absent,
        ),
    )
    .await
    .expect("unrelated sibling stays live")
    .unwrap();
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    held.rollback().await.unwrap();
    assert!(fs.get(&leaf).await.unwrap().is_none());
    tokio::time::timeout(
        Duration::from_secs(5),
        fs.put(
            &leaf,
            Entry::bytes(b"after cancellation".to_vec()),
            CasExpectation::Absent,
        ),
    )
    .await
    .expect("aborted wait does not retain its transaction locks")
    .unwrap();
    for index in 0..12 {
        let parent = VirtualPath::new(format!("/projects/namespace-directory-{index}")).unwrap();
        let child = VirtualPath::new(format!("{}/child", parent.as_str())).unwrap();
        let (file, directory) = tokio::join!(
            fs.put(
                &parent,
                Entry::bytes(b"file".to_vec()),
                CasExpectation::Absent
            ),
            fs.create_dir_all(&child)
        );
        assert_ne!(file.is_ok(), directory.is_ok());
        if directory.is_ok() {
            assert_eq!(
                fs.stat(&parent).await.unwrap().file_type,
                FileType::Directory
            );
        } else {
            assert_eq!(fs.get(&parent).await.unwrap().unwrap().entry.body, b"file");
        }
    }
    let system_directory = VirtualPath::new("/system/settings/namespace-directory/child").unwrap();
    fs.create_dir_all(&system_directory).await.unwrap();
    assert_eq!(
        fs.stat(&system_directory).await.unwrap().file_type,
        FileType::Directory
    );
    // Deliberate DB rejection must roll back both effects and lock ownership.
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE FUNCTION reject_namespace_fixture() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.path='/projects/namespace-rejected' THEN RAISE EXCEPTION 'fixture rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_namespace_fixture BEFORE INSERT OR UPDATE ON root_filesystem_entries FOR EACH ROW EXECUTE FUNCTION reject_namespace_fixture()").await.unwrap();
    let rejected = VirtualPath::new("/projects/namespace-rejected").unwrap();
    assert!(
        fs.put(
            &rejected,
            Entry::bytes(b"no effect".to_vec()),
            CasExpectation::Absent
        )
        .await
        .is_err()
    );
    assert!(fs.get(&rejected).await.unwrap().is_none());
    client.batch_execute("DROP TRIGGER reject_namespace_fixture ON root_filesystem_entries; DROP FUNCTION reject_namespace_fixture()").await.unwrap();
    tokio::time::timeout(
        Duration::from_secs(5),
        fs.put(
            &rejected,
            Entry::bytes(b"committed once".to_vec()),
            CasExpectation::Absent,
        ),
    )
    .await
    .expect("rejected transaction releases locks")
    .unwrap();
}

#[path = "../../../../brassclaw_filesystem/tests/support/metadata_contract.rs"]
mod filesystem_metadata_contract;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_filesystem_metadata_projection_works_without_payload_privileges() {
    use brassclaw_filesystem::{
        CasExpectation, Entry, Filter, Page, PostgresRootFilesystem, RecordKind, RootFilesystem,
    };
    use brassclaw_host_api::VirtualPath;
    use brassclaw_memory::{
        FilesystemMemoryDocumentRepository, MemoryDocumentPath, MemoryDocumentRepository,
        MemoryDocumentScope,
    };
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let filesystem = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    filesystem.run_migrations().await.unwrap();
    filesystem_metadata_contract::assert_metadata_contract(filesystem.clone()).await;
    let repository = FilesystemMemoryDocumentRepository::new(filesystem.clone());
    let scope = MemoryDocumentScope::new_with_agent(
        "metadata-owner",
        "operator",
        Some("agent"),
        Some("project"),
    )
    .unwrap();
    for name in ["keep.md", "large.md", "second.md"] {
        let path = MemoryDocumentPath::new_with_agent(
            "metadata-owner",
            "operator",
            Some("agent"),
            Some("project"),
            name,
        )
        .unwrap();
        let bytes = if name == "large.md" {
            vec![b'x'; 2 * 1024 * 1024]
        } else {
            b"payload".to_vec()
        };
        repository.write_document(&path, &bytes).await.unwrap();
    }
    let prefix = VirtualPath::new(
        "/memory/tenants/metadata-owner/users/operator/agents/agent/projects/project",
    )
    .unwrap();
    // Correct kind is insufficient for reserved sidecar paths to be documents.
    for relative in [
        "orphan.md.meta",
        "orphan.md.chunks/0",
        "orphan.md.versions/1",
    ] {
        filesystem
            .put(
                &VirtualPath::new(format!("{}/{relative}", prefix.as_str())).unwrap(),
                Entry::record(
                    RecordKind::new("memory_document").unwrap(),
                    &serde_json::Value::Null,
                )
                .unwrap(),
                CasExpectation::Absent,
            )
            .await
            .unwrap();
    }
    for (tenant, user, agent, project) in [
        ("other", "operator", "agent", "project"),
        ("metadata-owner", "other", "agent", "project"),
        ("metadata-owner", "operator", "other", "project"),
        ("metadata-owner", "operator", "agent", "other"),
    ] {
        let path = MemoryDocumentPath::new_with_agent(
            tenant,
            user,
            Some(agent),
            Some(project),
            "outside.md",
        )
        .unwrap();
        repository.write_document(&path, b"outside").await.unwrap();
    }
    let admin = rig.pool.get().await.unwrap();
    admin.batch_execute("CREATE ROLE metadata_projection_reader LOGIN; GRANT USAGE ON SCHEMA public TO metadata_projection_reader; GRANT SELECT(path,kind,is_dir) ON root_filesystem_entries TO metadata_projection_reader;").await.unwrap();
    let mut config: tokio_postgres::Config = rig.url.expose_secret().parse().unwrap();
    config.user("metadata_projection_reader");
    let manager = deadpool_postgres::Manager::new(config, tokio_postgres::NoTls);
    let pool = deadpool_postgres::Pool::builder(manager)
        .max_size(2)
        .runtime(deadpool_postgres::Runtime::Tokio1)
        .build()
        .unwrap();
    let limited = Arc::new(PostgresRootFilesystem::new(pool.clone()));
    let metadata = limited
        .query_metadata(&prefix, Page::default())
        .await
        .unwrap();
    assert!(
        metadata
            .iter()
            .any(|entry| entry.path.as_str().ends_with("/large.md"))
    );
    let large = VirtualPath::new(format!("{}/large.md", prefix.as_str())).unwrap();
    assert_eq!(
        limited
            .get_metadata(&large)
            .await
            .unwrap()
            .unwrap()
            .kind
            .unwrap()
            .as_str(),
        "memory_document"
    );
    assert!(
        limited.get(&large).await.is_err(),
        "full bodies are actually forbidden for this database role"
    );
    assert!(
        limited
            .query(&prefix, &Filter::All, Page::default())
            .await
            .is_err(),
        "ordinary full queries are actually forbidden"
    );
    let projected = FilesystemMemoryDocumentRepository::new(limited.clone());
    let paths = projected.list_documents(&scope).await.unwrap();
    assert_eq!(
        paths
            .iter()
            .map(|path| path.relative_path())
            .collect::<Vec<_>>(),
        vec!["keep.md", "large.md", "second.md"]
    );
    let key = brassclaw_filesystem::IndexKey::new("tenant_id").unwrap();
    assert!(
        limited
            .query_index_metadata(&large, &key, Page::default())
            .await
            .is_err(),
        "selected fields still require actual indexed-column read permission"
    );
    admin
        .batch_execute(
            "GRANT SELECT(indexed) ON root_filesystem_entries TO metadata_projection_reader",
        )
        .await
        .unwrap();
    let selected = limited
        .query_index_metadata(&large, &key, Page::default())
        .await
        .unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(
        selected[0].value,
        Some(brassclaw_filesystem::IndexValue::Text(
            "metadata-owner".into()
        ))
    );
    assert!(
        limited.get(&large).await.is_err(),
        "granting indexed access must not permit full-body reads"
    );
    admin.execute("UPDATE root_filesystem_entries SET indexed=jsonb_set(indexed,'{tenant_id}','null'::jsonb) WHERE path=$1", &[&large.as_str()]).await.unwrap();
    assert!(
        limited
            .query_index_metadata(&large, &key, Page::default())
            .await
            .is_err(),
        "malformed selected field must not become absent"
    );
    assert!(
        limited.get_metadata(&large).await.unwrap().is_some(),
        "unrequested indexed payload is not part of structural metadata"
    );
    // A bad header is still an error; do not hide it or read a body fallback.
    admin
        .execute(
            "UPDATE root_filesystem_entries SET kind='bad/kind' WHERE path=$1",
            &[&large.as_str()],
        )
        .await
        .unwrap();
    let error = limited.get_metadata(&large).await.unwrap_err();
    assert!(!error.to_string().contains("bad/kind"));
    assert!(projected.list_documents(&scope).await.is_err());
    pool.close();
}

#[path = "../../../../brassclaw_memory/tests/support/archive_contract.rs"]
mod memory_archive_contract;

#[tokio::test]
async fn native_memory_archive_projection_validates_counters_and_committed_effects() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let filesystem = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    filesystem.run_migrations().await.unwrap();
    memory_archive_contract::assert_archive_contract(filesystem).await;
}
