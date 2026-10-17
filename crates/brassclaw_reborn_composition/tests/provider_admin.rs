#![cfg(all(feature = "root-llm-provider", feature = "postgres"))]

#[path = "common/native_pg.rs"]
mod native_pg;

use brassclaw_reborn_composition::{RebornProviderAdmin, RebornProviderAdminError, RebornV1State};

#[tokio::test]
async fn provider_admin_reads_only_database_and_retains_read_failures() {
    let rig = native_pg::NativePostgres::start().await;
    let tenant = format!("admin-{}", uuid::Uuid::new_v4());
    let admin = RebornProviderAdmin::new((*rig.pool).clone(), &tenant);
    let empty = admin
        .list(None, false)
        .await
        .expect("empty authoritative catalogue");
    assert!(empty.providers.is_empty());
    let status = admin.status().await.expect("empty authoritative selection");
    let status_json = serde_json::to_value(status).unwrap();
    assert_eq!(status_json["routes"], "not-configured");
    assert_eq!(status_json["v1_state"], RebornV1State::NotUsed.as_str());
    assert!(status_json.get("config_file").is_none());

    let definition = serde_json::from_value(serde_json::json!({
        "id": "db-only", "aliases": ["database-alias"], "protocol": "open_ai_completions",
        "default_base_url": "http://127.0.0.1:8000/v1",
        "model_env": "DB_ONLY_MODEL", "default_model": "db-model", "description": "Database provider"
    })).unwrap();
    let repo = brassclaw_reborn_composition::PgProviderRepo::new((*rig.pool).clone(), &tenant);
    repo.upsert(definition).await.unwrap();
    use brassclaw_reborn_composition::db_config::{ConfigWriteContext, save_config_key};
    save_config_key(
        &rig.pool,
        &tenant,
        "llm.default.provider_id",
        "DATABASE-ALIAS",
        ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    save_config_key(
        &rig.pool,
        &tenant,
        "llm.default.model",
        "selected-db-model",
        ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let list = admin.list(Some("DATABASE-ALIAS"), true).await.unwrap();
    assert_eq!(list.providers.len(), 1);
    assert_eq!(list.providers[0].id, "db-only");
    assert!(list.providers[0].active);
    assert_eq!(
        list.providers[0].active_model.as_deref(),
        Some("selected-db-model")
    );
    let selected = admin.status().await.unwrap().default.unwrap();
    assert!(selected.provider_known);
    assert_eq!(selected.model.as_deref(), Some("selected-db-model"));
    match admin
        .list(Some("missing-provider"), false)
        .await
        .unwrap_err()
    {
        RebornProviderAdminError::UnknownProvider { provider, known } => {
            assert_eq!(provider, "missing-provider");
            assert_eq!(known, ["db-only"]);
        }
        other => panic!("wrong error: {other}"),
    }
    let client = rig.pool.get().await.unwrap();
    let collision: brassclaw_llm::ProviderDefinition = serde_json::from_value(serde_json::json!({
        "id": "alias-collision", "aliases": ["DATABASE-ALIAS"], "protocol": "open_ai_completions",
        "model_env": "COLLISION_MODEL", "default_model": "unused", "description": "Conflicting catalogue row"
    })).unwrap();
    repo.upsert(collision).await.unwrap();
    assert!(matches!(
        admin.status().await,
        Err(RebornProviderAdminError::LoadRegistry { .. })
    ));
    assert!(repo.delete("alias-collision").await.unwrap());
    assert!(
        admin
            .status()
            .await
            .unwrap()
            .default
            .unwrap()
            .provider_known
    );
    client.execute("UPDATE brassclaw_llm_providers SET definition=jsonb_set(definition,'{id}','\"wrong-identity\"'::jsonb) WHERE tenant_id=$1 AND id='db-only'", &[&tenant]).await.unwrap();
    assert!(matches!(
        admin.list(None, false).await,
        Err(RebornProviderAdminError::LoadRegistry { .. })
    ));
    client
        .execute(
            "UPDATE brassclaw_llm_providers SET definition='{}'::jsonb WHERE tenant_id=$1",
            &[&tenant],
        )
        .await
        .unwrap();
    assert!(matches!(
        admin.list(None, false).await,
        Err(RebornProviderAdminError::LoadRegistry { .. })
    ));
    assert!(matches!(
        admin.status().await,
        Err(RebornProviderAdminError::LoadRegistry { .. })
    ));
    client
        .batch_execute("ALTER TABLE brassclaw_config RENAME TO temporarily_hidden_config")
        .await
        .unwrap();
    assert!(matches!(
        admin.status().await,
        Err(RebornProviderAdminError::LoadDatabase { .. })
    ));
    client
        .batch_execute("ALTER TABLE temporarily_hidden_config RENAME TO brassclaw_config")
        .await
        .unwrap();
}
