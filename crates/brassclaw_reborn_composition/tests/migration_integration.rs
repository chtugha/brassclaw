//! Real embedded SQLite/PostgreSQL upgrade acceptance. No environment-based skip.
#![cfg(feature = "migrate-from-libsql")]

#[path = "common/native_pg.rs"]
mod native_pg;

use brassclaw_reborn_composition::migration;
use brassclaw_reborn_config::RebornHome;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn seed_libsql_then_migrate_asserts_all_rows_in_pg() {
    let rig = native_pg::NativePostgres::start().await;
    let directory = tempfile::tempdir().unwrap();
    let home = RebornHome::resolve_from_env_parts(
        Some(directory.path().as_os_str().to_os_string()),
        None,
        None,
    )
    .unwrap();
    std::fs::write(
        home.path().join("config.toml"),
        "[identity]\ntenant='test-tenant'\ndefault_owner='test-user'\n",
    )
    .unwrap();
    let provider = serde_json::json!({"id":"legacy-custom","protocol":"open_ai_completions","default_base_url":"http://127.0.0.1:8000/v1","base_url_required":true,"api_key_required":false,"model_env":"UNUSED_IMPORT_MODEL","default_model":"legacy-model","description":"Explicit import fixture"});
    std::fs::write(
        home.path().join("providers.json"),
        serde_json::to_vec(&vec![provider]).unwrap(),
    )
    .unwrap();
    std::fs::write(
        home.path().join("sempai_provider.json"),
        br#"{"provider_id":"legacy-custom","model":"legacy-model"}"#,
    )
    .unwrap();
    let key: Vec<u8> = (0..32).collect();
    std::fs::write(
        home.path().join(".reborn-local-dev-secrets-master-key"),
        format!("{}\n", hex::encode(&key)),
    )
    .unwrap();
    let source = home.path().join("reborn-local-dev.db");
    let database = libsql::Builder::new_local(&source).build().await.unwrap();
    let connection = database.connect().unwrap();
    connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
      CREATE TABLE safety_config(id TEXT,user_id TEXT,category TEXT,pattern TEXT,is_enabled INTEGER,is_default INTEGER,created_at TEXT,updated_at TEXT);
      INSERT INTO safety_config VALUES('safety-1','test-user','blocked_paths','/private',1,0,'2026-10-12T00:00:00Z','2026-10-12T00:00:00Z');
      CREATE TABLE settings(user_id TEXT,key TEXT,value TEXT,updated_at TEXT);
      INSERT INTO settings VALUES('test-user','provider_tokens:legacy-custom','{\"total_input\":1234}','2026-10-12T00:00:00Z');
      CREATE TABLE memory_docs(user_id TEXT,project_id TEXT,id TEXT,doc_type TEXT,title TEXT,content TEXT,source_thread_id TEXT,tags_json TEXT,metadata_json TEXT,created_at TEXT,updated_at TEXT);
      INSERT INTO memory_docs VALUES('test-user','default','doc-1','note','Retained title','Retained content',NULL,'[\"legacy\"]','{\"source\":\"legacy\"}','2026-10-12T00:00:00Z','2026-10-12T00:00:00Z');
      CREATE TABLE root_filesystem_entries(path TEXT,contents BLOB,is_dir INTEGER,content_type TEXT,kind TEXT,indexed TEXT,version INTEGER,created_at TEXT,updated_at TEXT);
      INSERT INTO root_filesystem_entries VALUES('/file',X'6869',0,'text/plain',NULL,'{}',3,'2026-10-12T00:00:00Z','2026-10-12T00:00:00Z');
      CREATE TABLE root_filesystem_index_specs(prefix TEXT,name TEXT,keys TEXT,kind TEXT);
      INSERT INTO root_filesystem_index_specs VALUES('/','legacy-index','[]','btree');
      CREATE TABLE root_filesystem_events(id INTEGER,path TEXT,payload BLOB,created_at TEXT);
      INSERT INTO root_filesystem_events VALUES(7,'/file',X'657674','2026-10-12T00:00:00Z');
      CREATE TABLE capability_permissions(tenant_id TEXT,capability_id TEXT,permission_mode TEXT,updated_at TEXT);
      INSERT INTO capability_permissions VALUES('test-tenant','legacy-capability','deny','2026-10-12T00:00:00Z');
      CREATE TABLE hooks_predicate_invocations(key_hash BLOB,scope_hash BLOB,event_id TEXT,recorded_at INTEGER);
      INSERT INTO hooks_predicate_invocations VALUES(zeroblob(32),zeroblob(32),'event-1',0);
      CREATE TABLE hooks_predicate_values(key_hash BLOB,scope_hash BLOB,event_id TEXT,value TEXT,recorded_at INTEGER);
      INSERT INTO hooks_predicate_values VALUES(zeroblob(32),zeroblob(32),'event-1','12.5',0);
      CREATE TABLE trigger_records(id TEXT,tenant_id TEXT,creator_user_id TEXT,name TEXT,trigger_kind TEXT,trigger_config TEXT,status TEXT,created_at TEXT);
      INSERT INTO trigger_records VALUES('trigger-1','test-tenant','test-user','Legacy trigger','cron','{}','active','2026-10-12T00:00:00Z');").await.unwrap();
    let report = migration::run_migration(&rig.pool, &home, "test-tenant", false)
        .await
        .unwrap();
    assert!(
        report.config_migrated
            && report.providers_migrated
            && report.sempai_migrated
            && report.secrets_master_migrated
            && report.libsql_db_migrated
            && report.boot_initialized_set
    );
    assert_eq!(report.paused_legacy_triggers, 1);
    let pg = rig.pool.get().await.unwrap();
    assert_eq!(pg.query_one("SELECT value FROM brassclaw_config WHERE tenant_id='test-tenant' AND key='identity.tenant'",&[]).await.unwrap().get::<_,String>(0),"test-tenant");
    assert_eq!(pg.query_one("SELECT value FROM brassclaw_config WHERE tenant_id='test-tenant' AND key='llm.sempai.model'",&[]).await.unwrap().get::<_,String>(0),"legacy-model");
    assert_eq!(pg.query_one("SELECT definition->>'default_model' FROM brassclaw_llm_providers WHERE tenant_id='test-tenant' AND id='legacy-custom'",&[]).await.unwrap().get::<_,String>(0),"legacy-model");
    assert_eq!(
        pg.query_one(
            "SELECT pattern FROM brassclaw_safety_config WHERE id='safety-1'",
            &[]
        )
        .await
        .unwrap()
        .get::<_, String>(0),
        "/private"
    );
    assert_eq!(pg.query_one("SELECT settings->>'total_input' FROM brassclaw_token_settings WHERE provider_id='legacy-custom'",&[]).await.unwrap().get::<_,String>(0),"1234");
    let doc=pg.query_one("SELECT title,content,tags,metadata,source_thread_id FROM brassclaw_memory_docs WHERE id='doc-1'",&[]).await.unwrap();
    assert_eq!(doc.get::<_, String>(0), "Retained title");
    assert_eq!(doc.get::<_, String>(1), "Retained content");
    assert_eq!(doc.get::<_, Vec<String>>(2), vec!["legacy"]);
    assert_eq!(
        doc.get::<_, serde_json::Value>(3),
        serde_json::json!({"source":"legacy"})
    );
    assert_eq!(doc.get::<_, Option<String>>(4), None);
    assert_eq!(
        pg.query_one(
            "SELECT contents FROM brassclaw_root_filesystem WHERE path='/file'",
            &[]
        )
        .await
        .unwrap()
        .get::<_, Vec<u8>>(0),
        b"hi"
    );
    assert_eq!(
        pg.query_one(
            "SELECT count(*) FROM brassclaw_root_filesystem_index_specs WHERE name='legacy-index'",
            &[]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        1
    );
    assert_eq!(
        pg.query_one(
            "SELECT seq FROM brassclaw_root_filesystem_events WHERE path='/file'",
            &[]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        7
    );
    assert_eq!(pg.query_one("SELECT permission_mode FROM brassclaw_capability_permissions WHERE capability_id='legacy-capability'",&[]).await.unwrap().get::<_,String>(0),"deny");
    assert_eq!(pg.query_one("SELECT extract(epoch FROM occurred_at)::bigint FROM hooks_predicate_invocations WHERE event_id='event-1'",&[]).await.unwrap().get::<_,i64>(0),0);
    assert_eq!(
        pg.query_one(
            "SELECT value::text FROM hooks_predicate_values WHERE event_id='event-1'",
            &[]
        )
        .await
        .unwrap()
        .get::<_, String>(0),
        "12.5"
    );
    assert_eq!(
        pg.query_one(
            "SELECT state FROM brassclaw_triggers WHERE trigger_id='trigger-1'",
            &[]
        )
        .await
        .unwrap()
        .get::<_, String>(0),
        "paused"
    );
    for name in [
        "config.toml",
        "providers.json",
        "sempai_provider.json",
        "reborn-local-dev.db",
        ".reborn-local-dev-secrets-master-key",
    ] {
        assert!(
            home.path().join(name).exists(),
            "source {name} must be retained"
        );
    }
    drop(pg);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dry_run_migration_writes_nothing() {
    let rig = native_pg::NativePostgres::start().await;
    let directory = tempfile::tempdir().unwrap();
    let home = RebornHome::resolve_from_env_parts(
        Some(directory.path().as_os_str().to_os_string()),
        None,
        None,
    )
    .unwrap();
    std::fs::write(
        home.path().join("config.toml"),
        "[identity]\ntenant='dry-tenant'\n",
    )
    .unwrap();
    assert!(
        migration::run_migration(&rig.pool, &home, "dry-tenant", true)
            .await
            .unwrap()
            .config_migrated
    );
    let pg = rig.pool.get().await.unwrap();
    assert_eq!(
        pg.query_one(
            "SELECT count(*) FROM brassclaw_config WHERE tenant_id='dry-tenant'",
            &[]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        0
    );
    assert!(std::fs::read_dir(home.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".migrated")
    }));
    drop(pg);
}
