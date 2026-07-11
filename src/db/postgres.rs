//! PostgreSQL backend for the Database trait.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use deadpool_postgres::{GenericClient as _, Pool};
use rust_decimal::Decimal;
use tokio_postgres::GenericClient as _;
use uuid::Uuid;

use crate::config::DatabaseConfig;
use crate::db::{
    ApiTokenRecord, CapabilityPermissionStore, ChannelPairingStore, Database,
    IdentityStore, PairingRequestRecord, SettingRow, SettingsStore,
    UserIdentityRecord, UserRecord, UserStore, WorkspaceStore,
};
use crate::db::tls;
use crate::error::{DatabaseError, WorkspaceError};
use crate::workspace::{
    ChunkWrite, DocumentVersion, MemoryChunk, MemoryDocument, Repository, SearchConfig,
    SearchResult, VersionSummary, WorkspaceEntry,
};

/// Greeting inserted into the first assistant thread for a new user.
const GREETING_SEED: &str = "Hello! I'm your AI assistant. How can I help you today?";

/// PostgreSQL database backend.
pub struct PgBackend {
    pool: Pool,
    repo: Repository,
}

impl PgBackend {
    /// Create a new PostgreSQL backend from configuration.
    pub async fn new(config: &DatabaseConfig) -> Result<Self, DatabaseError> {
        let mut cfg = deadpool_postgres::Config::new();
        cfg.url = Some(config.url().to_string());
        cfg.pool = Some(deadpool_postgres::PoolConfig {
            max_size: config.pool_size,
            ..Default::default()
        });
        let pool = tls::create_pool(&cfg, config.ssl_mode)
            .map_err(|e| DatabaseError::Pool(e.to_string()))?;
        // Test connection.
        let _ = pool.get().await?;
        let repo = Repository::new(pool.clone());
        Ok(Self { pool, repo })
    }

    /// Get a clone of the connection pool.
    ///
    /// Useful for sharing with components that still need raw pool access.
    pub fn pool(&self) -> Pool {
        self.pool.clone()
    }

    /// Get a pooled connection.
    async fn conn(&self) -> Result<deadpool_postgres::Object, DatabaseError> {
        self.pool.get().await.map_err(|e| DatabaseError::Pool(e.to_string()))
    }
}

// ==================== Row helpers ====================

fn row_to_user(row: &tokio_postgres::Row) -> UserRecord {
    UserRecord {
        id: row.get("id"),
        email: row.get("email"),
        display_name: row.get("display_name"),
        status: row.get("status"),
        role: row.get("role"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        last_login_at: row.get("last_login_at"),
        created_by: row.get("created_by"),
        metadata: row.get("metadata"),
    }
}

fn row_to_api_token(row: &tokio_postgres::Row) -> ApiTokenRecord {
    ApiTokenRecord {
        id: row.get("id"),
        user_id: row.get("user_id"),
        name: row.get("name"),
        token_prefix: row.get("token_prefix"),
        expires_at: row.get("expires_at"),
        last_used_at: row.get("last_used_at"),
        created_at: row.get("created_at"),
        revoked_at: row.get("revoked_at"),
    }
}

async fn seed_initial_assistant_thread(
    client: &impl tokio_postgres::GenericClient,
    user_id: &str,
    created_at: DateTime<Utc>,
) -> Result<(), DatabaseError> {
    let conversation_id = Uuid::new_v4();
    let message_id = Uuid::new_v4();
    let metadata = serde_json::json!({
        "thread_type": "assistant",
        "title": "Assistant",
    });
    client
        .execute(
            r#"
            INSERT INTO conversations (id, channel, user_id, metadata, source_channel, started_at, last_activity)
            VALUES ($1, 'gateway', $2, $3, 'gateway', $4, $4)
            "#,
            &[&conversation_id, &user_id, &metadata, &created_at],
        )
        .await?;
    client
        .execute(
            r#"
            INSERT INTO conversation_messages (id, conversation_id, role, content, created_at)
            VALUES ($1, $2, 'assistant', $3, $4)
            "#,
            &[&message_id, &conversation_id, &GREETING_SEED, &created_at],
        )
        .await?;
    Ok(())
}

// ==================== Database (supertrait) ====================

#[async_trait]
impl Database for PgBackend {
    async fn run_migrations(&self) -> Result<(), DatabaseError> {
        let mut client = self.conn().await?;
        crate::db::migration_fixup::run_postgres_migrations_with_fixup(&mut client).await
    }

    async fn migrate_default_owner(&self, owner_id: &str) -> Result<(), DatabaseError> {
        let mut client = self
            .pool()
            .get()
            .await
            .map_err(|e| DatabaseError::Pool(e.to_string()))?;
        let tx = client
            .transaction()
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;
        // Only tables with a real `user_id` column participate in the legacy
        // 'default' -> owner rewrite. `dynamic_tools` is intentionally excluded:
        // it is ownerless today and scoped by `scope`, not `user_id`.
        let tables = [
            "conversations",
            "memory_documents",
            "heartbeat_state",
            "secrets",
            "wasm_tools",
            "routines",
            "settings",
            "agent_jobs",
            "api_tokens",
        ];
        for table in &tables {
            tx.execute(
                &format!(
                    "UPDATE {} SET user_id = $1 WHERE user_id = 'default'",
                    table
                ),
                &[&owner_id],
            )
            .await
            .map_err(|e| DatabaseError::Query(format!("migrate_default_owner {table}: {e}")))?;
        }
        tx.commit()
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;
        Ok(())
    }
}

// ==================== SettingsStore ====================

#[async_trait]
impl SettingsStore for PgBackend {
    async fn get_setting(
        &self,
        user_id: &str,
        key: &str,
    ) -> Result<Option<serde_json::Value>, DatabaseError> {
        let conn = self.conn().await?;
        let row = conn
            .query_opt(
                "SELECT value FROM settings WHERE user_id = $1 AND key = $2",
                &[&user_id, &key],
            )
            .await?;
        Ok(row.map(|r| r.get("value")))
    }

    async fn get_setting_full(
        &self,
        user_id: &str,
        key: &str,
    ) -> Result<Option<SettingRow>, DatabaseError> {
        let conn = self.conn().await?;
        let row = conn
            .query_opt(
                "SELECT key, value, updated_at FROM settings WHERE user_id = $1 AND key = $2",
                &[&user_id, &key],
            )
            .await?;
        Ok(row.map(|r| SettingRow {
            key: r.get("key"),
            value: r.get("value"),
            updated_at: r.get("updated_at"),
        }))
    }

    async fn set_setting(
        &self,
        user_id: &str,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<(), DatabaseError> {
        let conn = self.conn().await?;
        conn.execute(
            r#"INSERT INTO settings (user_id, key, value, updated_at)
               VALUES ($1, $2, $3, NOW())
               ON CONFLICT (user_id, key) DO UPDATE SET
                   value = EXCLUDED.value, updated_at = NOW()"#,
            &[&user_id, &key, value],
        )
        .await?;
        Ok(())
    }

    async fn delete_setting(&self, user_id: &str, key: &str) -> Result<bool, DatabaseError> {
        let conn = self.conn().await?;
        let count = conn
            .execute(
                "DELETE FROM settings WHERE user_id = $1 AND key = $2",
                &[&user_id, &key],
            )
            .await?;
        Ok(count > 0)
    }

    async fn list_settings(&self, user_id: &str) -> Result<Vec<SettingRow>, DatabaseError> {
        let conn = self.conn().await?;
        let rows = conn
            .query(
                "SELECT key, value, updated_at FROM settings WHERE user_id = $1 ORDER BY key",
                &[&user_id],
            )
            .await?;
        Ok(rows
            .iter()
            .map(|r| SettingRow {
                key: r.get("key"),
                value: r.get("value"),
                updated_at: r.get("updated_at"),
            })
            .collect())
    }

    async fn get_all_settings(
        &self,
        user_id: &str,
    ) -> Result<HashMap<String, serde_json::Value>, DatabaseError> {
        let conn = self.conn().await?;
        let rows = conn
            .query(
                "SELECT key, value FROM settings WHERE user_id = $1",
                &[&user_id],
            )
            .await?;
        Ok(rows
            .iter()
            .map(|r| (r.get::<_, String>("key"), r.get::<_, serde_json::Value>("value")))
            .collect())
    }

    async fn set_all_settings(
        &self,
        user_id: &str,
        settings: &HashMap<String, serde_json::Value>,
    ) -> Result<(), DatabaseError> {
        let mut conn = self.conn().await?;
        let tx = conn.transaction().await?;
        for (key, value) in settings {
            tx.execute(
                r#"INSERT INTO settings (user_id, key, value, updated_at)
                   VALUES ($1, $2, $3, NOW())
                   ON CONFLICT (user_id, key) DO UPDATE SET
                       value = EXCLUDED.value, updated_at = NOW()"#,
                &[&user_id, &key, value],
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn has_settings(&self, user_id: &str) -> Result<bool, DatabaseError> {
        let conn = self.conn().await?;
        let row = conn
            .query_one(
                "SELECT COUNT(*) as cnt FROM settings WHERE user_id = $1",
                &[&user_id],
            )
            .await?;
        let count: i64 = row.get("cnt");
        Ok(count > 0)
    }
}

// ==================== WorkspaceStore ====================

#[async_trait]
impl WorkspaceStore for PgBackend {
    async fn get_document_by_path(
        &self,
        user_id: &str,
        agent_id: Option<Uuid>,
        path: &str,
    ) -> Result<MemoryDocument, WorkspaceError> {
        self.repo
            .get_document_by_path(user_id, agent_id, path)
            .await
    }

    async fn get_document_by_id(&self, id: Uuid) -> Result<MemoryDocument, WorkspaceError> {
        self.repo.get_document_by_id(id).await
    }

    async fn get_or_create_document_by_path(
        &self,
        user_id: &str,
        agent_id: Option<Uuid>,
        path: &str,
    ) -> Result<MemoryDocument, WorkspaceError> {
        self.repo
            .get_or_create_document_by_path(user_id, agent_id, path)
            .await
    }

    async fn update_document(&self, id: Uuid, content: &str) -> Result<(), WorkspaceError> {
        self.repo.update_document(id, content).await
    }

    async fn delete_document_by_path(
        &self,
        user_id: &str,
        agent_id: Option<Uuid>,
        path: &str,
    ) -> Result<(), WorkspaceError> {
        self.repo
            .delete_document_by_path(user_id, agent_id, path)
            .await
    }

    async fn list_directory(
        &self,
        user_id: &str,
        agent_id: Option<Uuid>,
        directory: &str,
    ) -> Result<Vec<WorkspaceEntry>, WorkspaceError> {
        self.repo.list_directory(user_id, agent_id, directory).await
    }

    async fn list_all_paths(
        &self,
        user_id: &str,
        agent_id: Option<Uuid>,
    ) -> Result<Vec<String>, WorkspaceError> {
        self.repo.list_all_paths(user_id, agent_id).await
    }

    async fn list_documents(
        &self,
        user_id: &str,
        agent_id: Option<Uuid>,
    ) -> Result<Vec<MemoryDocument>, WorkspaceError> {
        self.repo.list_documents(user_id, agent_id).await
    }

    async fn delete_chunks(&self, document_id: Uuid) -> Result<(), WorkspaceError> {
        self.repo.delete_chunks(document_id).await
    }

    async fn insert_chunk(
        &self,
        document_id: Uuid,
        chunk_index: i32,
        content: &str,
        embedding: Option<&[f32]>,
    ) -> Result<Uuid, WorkspaceError> {
        self.repo
            .insert_chunk(document_id, chunk_index, content, embedding)
            .await
    }

    async fn replace_chunks(
        &self,
        document_id: Uuid,
        chunks: &[ChunkWrite],
    ) -> Result<(), WorkspaceError> {
        self.repo.replace_chunks(document_id, chunks).await
    }

    async fn update_chunk_embedding(
        &self,
        chunk_id: Uuid,
        embedding: &[f32],
    ) -> Result<(), WorkspaceError> {
        self.repo.update_chunk_embedding(chunk_id, embedding).await
    }

    async fn get_chunks_without_embeddings(
        &self,
        user_id: &str,
        agent_id: Option<Uuid>,
        limit: usize,
    ) -> Result<Vec<MemoryChunk>, WorkspaceError> {
        self.repo
            .get_chunks_without_embeddings(user_id, agent_id, limit)
            .await
    }

    async fn hybrid_search(
        &self,
        user_id: &str,
        agent_id: Option<Uuid>,
        query: &str,
        embedding: Option<&[f32]>,
        config: &SearchConfig,
    ) -> Result<Vec<SearchResult>, WorkspaceError> {
        self.repo
            .hybrid_search(user_id, agent_id, query, embedding, config)
            .await
    }

    // Optimized multi-scope overrides using `ANY($1::text[])` SQL.

    async fn hybrid_search_multi(
        &self,
        user_ids: &[String],
        agent_id: Option<Uuid>,
        query: &str,
        embedding: Option<&[f32]>,
        config: &SearchConfig,
    ) -> Result<Vec<SearchResult>, WorkspaceError> {
        self.repo
            .hybrid_search_multi(user_ids, agent_id, query, embedding, config)
            .await
    }

    async fn list_all_paths_multi(
        &self,
        user_ids: &[String],
        agent_id: Option<Uuid>,
    ) -> Result<Vec<String>, WorkspaceError> {
        self.repo.list_all_paths_multi(user_ids, agent_id).await
    }

    async fn get_document_by_path_multi(
        &self,
        user_ids: &[String],
        agent_id: Option<Uuid>,
        path: &str,
    ) -> Result<MemoryDocument, WorkspaceError> {
        self.repo
            .get_document_by_path_multi(user_ids, agent_id, path)
            .await
    }

    async fn list_directory_multi(
        &self,
        user_ids: &[String],
        agent_id: Option<Uuid>,
        directory: &str,
    ) -> Result<Vec<WorkspaceEntry>, WorkspaceError> {
        self.repo
            .list_directory_multi(user_ids, agent_id, directory)
            .await
    }

    // ==================== Metadata ====================

    async fn update_document_metadata(
        &self,
        id: Uuid,
        metadata: &serde_json::Value,
    ) -> Result<(), WorkspaceError> {
        self.repo.update_document_metadata(id, metadata).await
    }

    async fn find_config_documents(
        &self,
        user_id: &str,
        agent_id: Option<Uuid>,
    ) -> Result<Vec<MemoryDocument>, WorkspaceError> {
        self.repo.find_config_documents(user_id, agent_id).await
    }

    // ==================== Versioning ====================

    async fn save_version(
        &self,
        document_id: Uuid,
        content: &str,
        content_hash: &str,
        changed_by: Option<&str>,
    ) -> Result<i32, WorkspaceError> {
        self.repo
            .save_version(document_id, content, content_hash, changed_by)
            .await
    }

    async fn get_version(
        &self,
        document_id: Uuid,
        version: i32,
    ) -> Result<DocumentVersion, WorkspaceError> {
        self.repo.get_version(document_id, version).await
    }

    async fn list_versions(
        &self,
        document_id: Uuid,
        limit: i64,
    ) -> Result<Vec<VersionSummary>, WorkspaceError> {
        self.repo.list_versions(document_id, limit).await
    }

    async fn get_latest_version_number(
        &self,
        document_id: Uuid,
    ) -> Result<Option<i32>, WorkspaceError> {
        self.repo.get_latest_version_number(document_id).await
    }

    async fn prune_versions(
        &self,
        document_id: Uuid,
        keep_count: i32,
    ) -> Result<u64, WorkspaceError> {
        self.repo.prune_versions(document_id, keep_count).await
    }
}

// ==================== UserStore ====================

#[async_trait]
impl UserStore for PgBackend {
    async fn create_user(&self, user: &UserRecord) -> Result<(), DatabaseError> {
        let mut conn = self.conn().await?;
        let tx = conn.transaction().await?;
        tx.execute(
            r#"INSERT INTO users (id, email, display_name, status, role, created_at, updated_at, last_login_at, created_by, metadata)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
            &[&user.id, &user.email, &user.display_name, &user.status, &user.role,
              &user.created_at, &user.updated_at, &user.last_login_at, &user.created_by, &user.metadata],
        ).await?;
        seed_initial_assistant_thread(&tx, &user.id, user.created_at).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn get_or_create_user(&self, user: UserRecord) -> Result<(), DatabaseError> {
        let mut conn = self.conn().await?;
        let tx = conn.transaction().await?;
        let rows = tx
            .execute(
                r#"INSERT INTO users (id, email, display_name, status, role, created_at, updated_at, last_login_at, created_by, metadata)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                   ON CONFLICT (id) DO NOTHING"#,
                &[&user.id, &user.email, &user.display_name, &user.status, &user.role,
                  &user.created_at, &user.updated_at, &user.last_login_at, &user.created_by, &user.metadata],
            )
            .await
            .map_err(|e| DatabaseError::Query(format!("get_or_create_user: {e}")))?;
        if rows > 0 {
            seed_initial_assistant_thread(&tx, &user.id, user.created_at).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn get_user(&self, id: &str) -> Result<Option<UserRecord>, DatabaseError> {
        let conn = self.conn().await?;
        let row = conn.query_opt(
            "SELECT id, email, display_name, status, role, created_at, updated_at, last_login_at, created_by, metadata FROM users WHERE id = $1",
            &[&id],
        ).await?;
        Ok(row.map(|r| row_to_user(&r)))
    }

    async fn get_user_by_email(&self, email: &str) -> Result<Option<UserRecord>, DatabaseError> {
        let conn = self.conn().await?;
        let row = conn.query_opt(
            "SELECT id, email, display_name, status, role, created_at, updated_at, last_login_at, created_by, metadata FROM users WHERE LOWER(email) = LOWER($1)",
            &[&email],
        ).await?;
        Ok(row.map(|r| row_to_user(&r)))
    }

    async fn list_users(&self, status: Option<&str>) -> Result<Vec<UserRecord>, DatabaseError> {
        let conn = self.conn().await?;
        let rows = match status {
            Some(s) => conn.query(
                "SELECT id, email, display_name, status, role, created_at, updated_at, last_login_at, created_by, metadata FROM users WHERE status = $1 ORDER BY created_at DESC",
                &[&s],
            ).await?,
            None => conn.query(
                "SELECT id, email, display_name, status, role, created_at, updated_at, last_login_at, created_by, metadata FROM users ORDER BY created_at DESC",
                &[],
            ).await?,
        };
        Ok(rows.iter().map(row_to_user).collect())
    }

    async fn update_user_status(&self, id: &str, status: &str) -> Result<(), DatabaseError> {
        let conn = self.conn().await?;
        conn.execute("UPDATE users SET status = $1, updated_at = NOW() WHERE id = $2", &[&status, &id]).await?;
        Ok(())
    }

    async fn update_user_role(&self, id: &str, role: &str) -> Result<(), DatabaseError> {
        let conn = self.conn().await?;
        conn.execute("UPDATE users SET role = $1, updated_at = NOW() WHERE id = $2", &[&role, &id]).await?;
        Ok(())
    }

    async fn update_user_profile(
        &self,
        id: &str,
        display_name: &str,
        metadata: &serde_json::Value,
    ) -> Result<(), DatabaseError> {
        let conn = self.conn().await?;
        conn.execute(
            "UPDATE users SET display_name = $1, metadata = $2, updated_at = NOW() WHERE id = $3",
            &[&display_name, metadata, &id],
        ).await?;
        Ok(())
    }

    async fn record_login(&self, id: &str) -> Result<(), DatabaseError> {
        let conn = self.conn().await?;
        conn.execute("UPDATE users SET last_login_at = NOW(), updated_at = NOW() WHERE id = $1", &[&id]).await?;
        Ok(())
    }

    async fn create_api_token(
        &self,
        user_id: &str,
        name: &str,
        token_hash: &[u8; 32],
        token_prefix: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<ApiTokenRecord, DatabaseError> {
        let conn = self.conn().await?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        conn.execute(
            r#"INSERT INTO api_tokens (id, user_id, token_hash, token_prefix, name, expires_at, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            &[&id, &user_id, &token_hash.as_slice(), &token_prefix, &name, &expires_at, &now],
        ).await?;
        Ok(ApiTokenRecord { id, user_id: user_id.to_string(), name: name.to_string(),
            token_prefix: token_prefix.to_string(), expires_at, last_used_at: None, created_at: now, revoked_at: None })
    }

    async fn list_api_tokens(&self, user_id: &str) -> Result<Vec<ApiTokenRecord>, DatabaseError> {
        let conn = self.conn().await?;
        let rows = conn.query(
            "SELECT id, user_id, name, token_prefix, expires_at, last_used_at, created_at, revoked_at FROM api_tokens WHERE user_id = $1 ORDER BY created_at DESC",
            &[&user_id],
        ).await?;
        Ok(rows.iter().map(row_to_api_token).collect())
    }

    async fn revoke_api_token(&self, token_id: Uuid, user_id: &str) -> Result<bool, DatabaseError> {
        let conn = self.conn().await?;
        let count = conn.execute(
            "UPDATE api_tokens SET revoked_at = NOW() WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL",
            &[&token_id, &user_id],
        ).await?;
        Ok(count > 0)
    }

    async fn authenticate_token(
        &self,
        token_hash: &[u8; 32],
    ) -> Result<Option<(ApiTokenRecord, UserRecord)>, DatabaseError> {
        let conn = self.conn().await?;
        let row = conn.query_opt(
            r#"SELECT t.id, t.user_id, t.name, t.token_prefix, t.expires_at, t.last_used_at, t.created_at, t.revoked_at,
                      u.id as u_id, u.email, u.display_name, u.status, u.role, u.created_at as u_created_at, u.updated_at, u.last_login_at, u.created_by, u.metadata
               FROM api_tokens t
               JOIN users u ON t.user_id = u.id
               WHERE t.token_hash = $1
                 AND t.revoked_at IS NULL
                 AND (t.expires_at IS NULL OR t.expires_at > NOW())
                 AND u.status = 'active'"#,
            &[&token_hash.as_slice()],
        ).await?;
        Ok(row.map(|r| {
            let token = ApiTokenRecord {
                id: r.get("id"), user_id: r.get("user_id"), name: r.get("name"),
                token_prefix: r.get("token_prefix"), expires_at: r.get("expires_at"),
                last_used_at: r.get("last_used_at"), created_at: r.get("created_at"),
                revoked_at: r.get("revoked_at"),
            };
            let user = UserRecord {
                id: r.get("u_id"), email: r.get("email"), display_name: r.get("display_name"),
                status: r.get("status"), role: r.get("role"), created_at: r.get("u_created_at"),
                updated_at: r.get("updated_at"), last_login_at: r.get("last_login_at"),
                created_by: r.get("created_by"), metadata: r.get("metadata"),
            };
            (token, user)
        }))
    }

    async fn record_token_usage(&self, token_id: Uuid) -> Result<(), DatabaseError> {
        let conn = self.conn().await?;
        conn.execute("UPDATE api_tokens SET last_used_at = NOW() WHERE id = $1", &[&token_id]).await?;
        Ok(())
    }

    async fn has_any_users(&self) -> Result<bool, DatabaseError> {
        let conn = self.conn().await?;
        let row = conn.query_one("SELECT EXISTS(SELECT 1 FROM users LIMIT 1) as has_users", &[]).await?;
        Ok(row.get("has_users"))
    }

    async fn delete_user(&self, id: &str) -> Result<bool, DatabaseError> {
        let mut conn = self.conn().await?;
        let tx = conn.transaction().await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;
        for table in &["settings", "heartbeat_state", "tool_rate_limit_state", "secret_usage_log",
            "leak_detection_events", "secrets", "wasm_tools", "routines", "memory_documents",
            "conversations", "user_identities"] {
            tx.execute(&format!("DELETE FROM {table} WHERE user_id = $1"), &[&id])
                .await.map_err(|e| DatabaseError::Query(e.to_string()))?;
        }
        tx.execute("DELETE FROM job_events WHERE job_id IN (SELECT id FROM agent_jobs WHERE user_id = $1)", &[&id])
            .await.map_err(|e| DatabaseError::Query(e.to_string()))?;
        tx.execute("DELETE FROM agent_jobs WHERE user_id = $1", &[&id])
            .await.map_err(|e| DatabaseError::Query(e.to_string()))?;
        tx.execute("UPDATE users SET created_by = NULL WHERE created_by = $1", &[&id])
            .await.map_err(|e| DatabaseError::Query(e.to_string()))?;
        let result = tx.execute("DELETE FROM users WHERE id = $1", &[&id])
            .await.map_err(|e| DatabaseError::Query(e.to_string()))?;
        tx.commit().await.map_err(|e| DatabaseError::Query(e.to_string()))?;
        Ok(result > 0)
    }

    async fn user_usage_stats(
        &self,
        user_id: Option<&str>,
        since: DateTime<Utc>,
    ) -> Result<Vec<crate::db::UserUsageStats>, DatabaseError> {
        let conn = self.conn().await?;
        let rows = if let Some(uid) = user_id {
            conn.query(
                r#"SELECT COALESCE(j.user_id, c.user_id) as user_id, l.model, COUNT(*) as call_count,
                          COALESCE(SUM(l.input_tokens), 0) as input_tokens,
                          COALESCE(SUM(l.output_tokens), 0) as output_tokens,
                          COALESCE(SUM(l.cost), 0) as total_cost
                   FROM llm_calls l
                   LEFT JOIN agent_jobs j ON l.job_id = j.id
                   LEFT JOIN conversations c ON l.conversation_id = c.id
                   WHERE l.created_at >= $1 AND COALESCE(j.user_id, c.user_id) = $2
                   GROUP BY COALESCE(j.user_id, c.user_id), l.model ORDER BY total_cost DESC"#,
                &[&since, &uid],
            ).await?
        } else {
            conn.query(
                r#"SELECT COALESCE(j.user_id, c.user_id) as user_id, l.model, COUNT(*) as call_count,
                          COALESCE(SUM(l.input_tokens), 0) as input_tokens,
                          COALESCE(SUM(l.output_tokens), 0) as output_tokens,
                          COALESCE(SUM(l.cost), 0) as total_cost
                   FROM llm_calls l
                   LEFT JOIN agent_jobs j ON l.job_id = j.id
                   LEFT JOIN conversations c ON l.conversation_id = c.id
                   WHERE l.created_at >= $1
                   GROUP BY COALESCE(j.user_id, c.user_id), l.model ORDER BY total_cost DESC"#,
                &[&since],
            ).await?
        };
        Ok(rows.iter().map(|r| crate::db::UserUsageStats {
            user_id: r.get("user_id"), model: r.get("model"), call_count: r.get("call_count"),
            input_tokens: r.get("input_tokens"), output_tokens: r.get("output_tokens"),
            total_cost: r.get("total_cost"),
        }).collect())
    }

    async fn user_summary_stats(
        &self,
        user_id: Option<&str>,
    ) -> Result<Vec<crate::db::UserSummaryStats>, DatabaseError> {
        let conn = self.conn().await?;
        let rows = if let Some(uid) = user_id {
            conn.query(
                r#"SELECT COALESCE(j.user_id, c.user_id) AS user_id,
                          COUNT(DISTINCT j.id) AS job_count,
                          COALESCE(SUM(l.cost), 0) AS total_cost,
                          MAX(l.created_at) AS last_active_at
                   FROM llm_calls l
                   LEFT JOIN agent_jobs j ON l.job_id = j.id
                   LEFT JOIN conversations c ON l.conversation_id = c.id
                   WHERE COALESCE(j.user_id, c.user_id) = $1
                   GROUP BY COALESCE(j.user_id, c.user_id)"#,
                &[&uid],
            ).await?
        } else {
            conn.query(
                r#"SELECT COALESCE(j.user_id, c.user_id) AS user_id,
                          COUNT(DISTINCT j.id) AS job_count,
                          COALESCE(SUM(l.cost), 0) AS total_cost,
                          MAX(l.created_at) AS last_active_at
                   FROM llm_calls l
                   LEFT JOIN agent_jobs j ON l.job_id = j.id
                   LEFT JOIN conversations c ON l.conversation_id = c.id
                   GROUP BY COALESCE(j.user_id, c.user_id)"#,
                &[],
            ).await?
        };
        Ok(rows.iter().map(|r| crate::db::UserSummaryStats {
            user_id: r.get("user_id"), job_count: r.get("job_count"),
            total_cost: r.get("total_cost"), last_active_at: r.get("last_active_at"),
        }).collect())
    }

    async fn admin_usage_summary(
        &self,
        since: DateTime<Utc>,
    ) -> Result<crate::db::AdminUsageSummary, DatabaseError> {
        let conn = self.conn().await?;
        let row = conn.query_one(
            r#"SELECT (SELECT COUNT(*) FROM users) AS total_users,
                      (SELECT COUNT(*) FROM users WHERE status = 'active') AS active_users,
                      (SELECT COUNT(*) FROM users WHERE status = 'suspended') AS suspended_users,
                      (SELECT COUNT(*) FROM users WHERE role = 'admin') AS admin_users,
                      (SELECT COUNT(*) FROM agent_jobs) AS total_jobs,
                      recent.llm_calls, recent.input_tokens, recent.output_tokens, recent.usage_cost
               FROM (SELECT COUNT(*) AS llm_calls,
                           COALESCE(SUM(input_tokens), 0) AS input_tokens,
                           COALESCE(SUM(output_tokens), 0) AS output_tokens,
                           COALESCE(SUM(cost), 0::numeric) AS usage_cost
                    FROM llm_calls WHERE created_at >= $1) recent"#,
            &[&since],
        ).await?;
        Ok(crate::db::AdminUsageSummary {
            total_users: row.get("total_users"), active_users: row.get("active_users"),
            suspended_users: row.get("suspended_users"), admin_users: row.get("admin_users"),
            total_jobs: row.get("total_jobs"), llm_calls: row.get("llm_calls"),
            input_tokens: row.get("input_tokens"), output_tokens: row.get("output_tokens"),
            usage_cost: row.get("usage_cost"),
        })
    }

    async fn create_user_with_token(
        &self,
        user: &UserRecord,
        token_name: &str,
        token_hash: &[u8; 32],
        token_prefix: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<ApiTokenRecord, DatabaseError> {
        let mut conn = self.conn().await?;
        let tx = conn.transaction().await?;
        tx.execute(
            r#"INSERT INTO users (id, email, display_name, status, role, created_at, updated_at, last_login_at, created_by, metadata)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
            &[&user.id, &user.email, &user.display_name, &user.status, &user.role,
              &user.created_at, &user.updated_at, &user.last_login_at, &user.created_by, &user.metadata],
        ).await?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        tx.execute(
            r#"INSERT INTO api_tokens (id, user_id, token_hash, token_prefix, name, expires_at, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            &[&id, &user.id, &token_hash.as_slice(), &token_prefix, &token_name, &expires_at, &now],
        ).await?;
        seed_initial_assistant_thread(&tx, &user.id, user.created_at).await?;
        tx.commit().await?;
        Ok(ApiTokenRecord { id, user_id: user.id.clone(), name: token_name.to_string(),
            token_prefix: token_prefix.to_string(), expires_at, last_used_at: None,
            created_at: now, revoked_at: None })
    }
}

// ==================== ChannelPairingStore ====================

#[async_trait]
impl ChannelPairingStore for PgBackend {
    async fn resolve_channel_identity(
        &self,
        channel: &str,
        external_id: &str,
    ) -> Result<Option<crate::ownership::UserId>, DatabaseError> {
        use crate::ownership::{UserId, UserRole};
        let channel = crate::pairing::normalize_channel_name(channel);
        let client = self
            .pool()
            .get()
            .await
            .map_err(|e| DatabaseError::Pool(e.to_string()))?;
        let row = client
            .query_opt(
                "SELECT ci.owner_id, u.role
                 FROM channel_identities ci
                 JOIN users u ON u.id = ci.owner_id
                 WHERE ci.channel = $1 AND ci.external_id = $2
                   AND u.status = 'active'",
                &[&channel, &external_id],
            )
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;

        Ok(row.map(|r| {
            let owner_id: String = r.get(0);
            let role_str: String = r.get(1);
            let role = UserRole::from_db_role(&role_str);
            UserId::from_trusted(owner_id, role)
        }))
    }

    async fn read_allow_from(&self, channel: &str) -> Result<Vec<String>, DatabaseError> {
        let channel = crate::pairing::normalize_channel_name(channel);
        let client = self
            .pool()
            .get()
            .await
            .map_err(|e| DatabaseError::Pool(e.to_string()))?;
        let rows = client
            .query(
                "SELECT ci.external_id
                 FROM channel_identities ci
                 JOIN users u ON u.id = ci.owner_id
                 WHERE ci.channel = $1
                   AND u.status = 'active'
                 ORDER BY ci.external_id ASC",
                &[&channel],
            )
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;

        Ok(rows.into_iter().map(|row| row.get(0)).collect())
    }

    async fn resolve_channel_external_id_for_owner(
        &self,
        channel: &str,
        owner_id: &str,
    ) -> Result<Option<String>, DatabaseError> {
        let channel = crate::pairing::normalize_channel_name(channel);
        let client = self
            .pool()
            .get()
            .await
            .map_err(|e| DatabaseError::Pool(e.to_string()))?;
        let row = client
            .query_opt(
                "SELECT ci.external_id
                 FROM channel_identities ci
                 LEFT JOIN users u ON u.id = ci.owner_id
                 WHERE ci.channel = $1
                   AND ci.owner_id = $2
                   AND (u.id IS NULL OR u.status = 'active')
                 ORDER BY ci.external_id ASC
                 LIMIT 1",
                &[&channel, &owner_id],
            )
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;

        Ok(row.map(|r| r.get(0)))
    }

    async fn upsert_pairing_request(
        &self,
        channel: &str,
        external_id: &str,
        meta: Option<serde_json::Value>,
    ) -> Result<PairingRequestRecord, DatabaseError> {
        let channel = crate::pairing::normalize_channel_name(channel);
        let mut client = self
            .pool()
            .get()
            .await
            .map_err(|e| DatabaseError::Pool(e.to_string()))?;
        let tx = client
            .transaction()
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;

        // Serialize upserts for the same normalized sender key so PostgreSQL
        // preserves the single-live-code guarantee that libSQL gets from
        // BEGIN IMMEDIATE.
        let lock_key = format!(
            "{}:{}:{}:{}",
            channel.len(),
            channel,
            external_id.len(),
            external_id
        );
        tx.query(
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            &[&lock_key],
        )
        .await
        .map_err(|e| DatabaseError::Query(e.to_string()))?;

        tx.execute(
            "UPDATE pairing_requests
             SET expires_at = NOW()
             WHERE channel = $1 AND external_id = $2
               AND approved_at IS NULL AND expires_at > NOW()",
            &[&channel, &external_id],
        )
        .await
        .map_err(|e| DatabaseError::Query(e.to_string()))?;

        let expires_at = chrono::Utc::now() + chrono::Duration::minutes(15);
        let meta_json: Option<serde_json::Value> = meta;

        // Retry loop: regenerate code on UNIQUE violation (code collision)
        for attempt in 0..3 {
            let code = crate::db::generate_pairing_code();
            match tx
                .query_one(
                    "INSERT INTO pairing_requests (id, channel, external_id, code, meta, expires_at)
                     VALUES (gen_random_uuid(), $1, $2, $3, $4, $5)
                     RETURNING id, channel, external_id, code, created_at, expires_at",
                    &[&channel, &external_id, &code, &meta_json, &expires_at],
                )
                .await
            {
                Ok(row) => {
                    tx.commit()
                        .await
                        .map_err(|e| DatabaseError::Query(e.to_string()))?;
                    return Ok(PairingRequestRecord {
                        id: row.get(0),
                        channel: row.get(1),
                        external_id: row.get(2),
                        code: row.get(3),
                        created: true,
                        created_at: row.get(4),
                        expires_at: row.get(5),
                    });
                }
                Err(e) => {
                    let is_unique = e
                        .code()
                        .is_some_and(|c| *c == tokio_postgres::error::SqlState::UNIQUE_VIOLATION);
                    if attempt < 2 && is_unique {
                        continue;
                    }
                    return Err(DatabaseError::Query(e.to_string()));
                }
            }
        }

        Err(DatabaseError::Query(
            "failed to generate unique pairing code after 3 attempts".to_string(),
        ))
    }

    async fn approve_pairing(
        &self,
        channel: &str,
        code: &str,
        owner_id: &str,
    ) -> Result<crate::db::PairingApprovalRecord, DatabaseError> {
        let channel = crate::pairing::normalize_channel_name(channel);
        let mut client = self
            .pool()
            .get()
            .await
            .map_err(|e| DatabaseError::Pool(e.to_string()))?;
        let tx = client
            .transaction()
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;

        let row = tx
            .query_opt(
                "SELECT id, channel, external_id FROM pairing_requests
                 WHERE UPPER(code) = UPPER($1)
                   AND channel = $2
                   AND approved_at IS NULL
                   AND expires_at > NOW()
                 FOR UPDATE",
                &[&code, &channel],
            )
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?
            .ok_or_else(|| DatabaseError::NotFound {
                entity: "pairing_request".to_string(),
                id: code.to_string(),
            })?;

        let req_id: uuid::Uuid = row.get(0);
        let channel: String = row.get(1);
        let external_id: String = row.get(2);
        let previous_owner_id = tx
            .query_opt(
                "SELECT owner_id
                 FROM channel_identities
                 WHERE channel = $1 AND external_id = $2
                 FOR UPDATE",
                &[&channel, &external_id],
            )
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?
            .map(|row| row.get(0));

        tx.execute(
            "UPDATE pairing_requests SET owner_id = $1, approved_at = NOW() WHERE id = $2",
            &[&owner_id, &req_id],
        )
        .await
        .map_err(|e| DatabaseError::Query(e.to_string()))?;

        tx.execute(
            "INSERT INTO channel_identities (id, owner_id, channel, external_id)
             VALUES (gen_random_uuid(), $1, $2, $3)
             ON CONFLICT (channel, external_id) DO UPDATE SET owner_id = $1",
            &[&owner_id, &channel, &external_id],
        )
        .await
        .map_err(|e| DatabaseError::Query(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;
        Ok(crate::db::PairingApprovalRecord {
            request_id: req_id,
            channel,
            external_id,
            owner_id: owner_id.to_string(),
            previous_owner_id,
        })
    }

    async fn revert_pairing_approval(
        &self,
        approval: &crate::db::PairingApprovalRecord,
    ) -> Result<(), DatabaseError> {
        let mut client = self
            .pool()
            .get()
            .await
            .map_err(|e| DatabaseError::Pool(e.to_string()))?;
        let tx = client
            .transaction()
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;

        let updated = tx
            .execute(
                "UPDATE pairing_requests
             SET owner_id = NULL, approved_at = NULL
             WHERE id = $1 AND owner_id = $2 AND approved_at IS NOT NULL",
                &[&approval.request_id, &approval.owner_id],
            )
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;

        if updated == 0 {
            return Err(DatabaseError::NotFound {
                entity: "pairing_approval".to_string(),
                id: approval.request_id.to_string(),
            });
        }

        if let Some(previous_owner_id) = approval.previous_owner_id.as_ref() {
            tx.execute(
                "INSERT INTO channel_identities (id, owner_id, channel, external_id)
                 VALUES (gen_random_uuid(), $1, $2, $3)
                 ON CONFLICT (channel, external_id) DO UPDATE SET owner_id = $1",
                &[previous_owner_id, &approval.channel, &approval.external_id],
            )
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;
        } else {
            tx.execute(
                "DELETE FROM channel_identities
                 WHERE channel = $1 AND external_id = $2 AND owner_id = $3",
                &[&approval.channel, &approval.external_id, &approval.owner_id],
            )
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))
    }

    async fn list_pending_pairings(
        &self,
        channel: &str,
    ) -> Result<Vec<PairingRequestRecord>, DatabaseError> {
        let channel = crate::pairing::normalize_channel_name(channel);
        let client = self
            .pool()
            .get()
            .await
            .map_err(|e| DatabaseError::Pool(e.to_string()))?;
        let rows = client
            .query(
                "SELECT id, channel, external_id, code, created_at, expires_at
                 FROM pairing_requests
                 WHERE channel = $1 AND approved_at IS NULL AND expires_at > NOW()
                 ORDER BY created_at ASC",
                &[&channel],
            )
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| PairingRequestRecord {
                id: r.get(0),
                channel: r.get(1),
                external_id: r.get(2),
                code: r.get(3),
                created: false,
                created_at: r.get(4),
                expires_at: r.get(5),
            })
            .collect())
    }

    async fn remove_channel_identity(
        &self,
        channel: &str,
        external_id: &str,
    ) -> Result<(), DatabaseError> {
        let channel = crate::pairing::normalize_channel_name(channel);
        let client = self
            .pool()
            .get()
            .await
            .map_err(|e| DatabaseError::Pool(e.to_string()))?;
        client
            .execute(
                "DELETE FROM channel_identities WHERE channel = $1 AND external_id = $2",
                &[&channel, &external_id],
            )
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;
        Ok(())
    }

    async fn create_channel_identity(
        &self,
        channel: &str,
        external_id: &str,
        owner_id: &str,
    ) -> Result<(), DatabaseError> {
        let channel = crate::pairing::normalize_channel_name(channel);
        let client = self
            .pool()
            .get()
            .await
            .map_err(|e| DatabaseError::Pool(e.to_string()))?;
        client
            .execute(
                "INSERT INTO channel_identities (owner_id, channel, external_id)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (channel, external_id)
                 DO UPDATE SET owner_id = $1",
                &[&owner_id, &channel, &external_id],
            )
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;
        Ok(())
    }
}

// ==================== IdentityStore ====================

fn row_to_identity(row: &tokio_postgres::Row) -> UserIdentityRecord {
    UserIdentityRecord {
        id: row.get("id"),
        user_id: row.get("user_id"),
        provider: row.get("provider"),
        provider_user_id: row.get("provider_user_id"),
        email: row.get("email"),
        email_verified: row.get("email_verified"),
        display_name: row.get("display_name"),
        avatar_url: row.get("avatar_url"),
        raw_profile: row.get("raw_profile"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

#[async_trait]
impl IdentityStore for PgBackend {
    async fn get_identity_by_provider(
        &self,
        provider: &str,
        provider_user_id: &str,
    ) -> Result<Option<UserIdentityRecord>, DatabaseError> {
        let conn = self.store.pool().get().await?;
        let row = conn
            .query_opt(
                "SELECT id, user_id, provider, provider_user_id, email, email_verified, \
                 display_name, avatar_url, raw_profile, created_at, updated_at \
                 FROM user_identities WHERE provider = $1 AND provider_user_id = $2",
                &[&provider, &provider_user_id],
            )
            .await?;
        Ok(row.as_ref().map(row_to_identity))
    }

    async fn list_identities_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<UserIdentityRecord>, DatabaseError> {
        let conn = self.store.pool().get().await?;
        let rows = conn
            .query(
                "SELECT id, user_id, provider, provider_user_id, email, email_verified, \
                 display_name, avatar_url, raw_profile, created_at, updated_at \
                 FROM user_identities WHERE user_id = $1 ORDER BY created_at",
                &[&user_id],
            )
            .await?;
        Ok(rows.iter().map(row_to_identity).collect())
    }

    async fn create_identity(&self, identity: &UserIdentityRecord) -> Result<(), DatabaseError> {
        let conn = self.store.pool().get().await?;
        conn.execute(
            "INSERT INTO user_identities \
             (id, user_id, provider, provider_user_id, email, email_verified, \
              display_name, avatar_url, raw_profile, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
            &[
                &identity.id,
                &identity.user_id,
                &identity.provider,
                &identity.provider_user_id,
                &identity.email,
                &identity.email_verified,
                &identity.display_name,
                &identity.avatar_url,
                &identity.raw_profile,
                &identity.created_at,
                &identity.updated_at,
            ],
        )
        .await?;
        Ok(())
    }

    async fn update_identity_profile(
        &self,
        provider: &str,
        provider_user_id: &str,
        display_name: Option<&str>,
        avatar_url: Option<&str>,
    ) -> Result<(), DatabaseError> {
        let conn = self.store.pool().get().await?;
        conn.execute(
            "UPDATE user_identities SET display_name = COALESCE($3, display_name), \
             avatar_url = COALESCE($4, avatar_url), updated_at = NOW() \
             WHERE provider = $1 AND provider_user_id = $2",
            &[&provider, &provider_user_id, &display_name, &avatar_url],
        )
        .await?;
        Ok(())
    }

    async fn find_identity_by_verified_email(
        &self,
        email: &str,
    ) -> Result<Option<UserIdentityRecord>, DatabaseError> {
        let conn = self.store.pool().get().await?;
        let row = conn
            .query_opt(
                "SELECT id, user_id, provider, provider_user_id, email, email_verified, \
                 display_name, avatar_url, raw_profile, created_at, updated_at \
                 FROM user_identities WHERE LOWER(email) = LOWER($1) AND email_verified = true LIMIT 1",
                &[&email],
            )
            .await?;
        Ok(row.as_ref().map(row_to_identity))
    }

    async fn create_user_with_identity(
        &self,
        user: &UserRecord,
        identity: &UserIdentityRecord,
    ) -> Result<(), DatabaseError> {
        let mut conn = self.conn().await?;
        let tx = conn.transaction().await?;

        tx.execute(
            "INSERT INTO users (id, email, display_name, status, role, created_at, \
             updated_at, last_login_at, created_by, metadata) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            &[
                &user.id,
                &user.email,
                &user.display_name,
                &user.status,
                &user.role,
                &user.created_at,
                &user.updated_at,
                &user.last_login_at,
                &user.created_by,
                &user.metadata,
            ],
        )
        .await?;

        tx.execute(
            "INSERT INTO user_identities \
             (id, user_id, provider, provider_user_id, email, email_verified, \
              display_name, avatar_url, raw_profile, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
            &[
                &identity.id,
                &identity.user_id,
                &identity.provider,
                &identity.provider_user_id,
                &identity.email,
                &identity.email_verified,
                &identity.display_name,
                &identity.avatar_url,
                &identity.raw_profile,
                &identity.created_at,
                &identity.updated_at,
            ],
        )
        .await?;

        // Atomically promote to admin if this is the only user in the table.
        // Under READ COMMITTED, two concurrent transactions could both see
        // COUNT(*)=1 (each sees its own uncommitted insert). Use an advisory
        // lock to serialize the first-user election across transactions.
        tx.execute(
            "SELECT pg_advisory_xact_lock(hashtext('first_user_admin_election'))",
            &[],
        )
        .await?;
        tx.execute(
            "UPDATE users SET role = 'admin' \
             WHERE id = $1 AND (SELECT COUNT(*) FROM users) = 1",
            &[&user.id],
        )
        .await?;

        Store::seed_initial_assistant_thread(&tx, &user.id, user.created_at).await?;

        tx.commit().await?;
        Ok(())
    }
}

// ==================== CapabilityPermissionStore ====================

#[async_trait]
impl CapabilityPermissionStore for PgBackend {
    async fn get_capability_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<Option<brassclaw_host_api::PermissionMode>, DatabaseError> {
        let conn = self.pool().get().await?;
        let row = conn
            .query_opt(
                "SELECT permission_mode FROM capability_permissions \
                 WHERE tenant_id = $1 AND capability_id = $2",
                &[&tenant_id, &capability_id],
            )
            .await?;

        match row {
            Some(row) => {
                let mode_str: String = row.get(0);
                let mode = match mode_str.as_str() {
                    "allow" => brassclaw_host_api::PermissionMode::Allow,
                    "ask" => brassclaw_host_api::PermissionMode::Ask,
                    "deny" => brassclaw_host_api::PermissionMode::Deny,
                    _ => {
                        return Err(DatabaseError::Query(format!(
                            "invalid permission_mode: {}",
                            mode_str
                        )));
                    }
                };
                Ok(Some(mode))
            }
            None => Ok(None),
        }
    }

    async fn set_capability_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
        mode: brassclaw_host_api::PermissionMode,
    ) -> Result<(), DatabaseError> {
        let conn = self.pool().get().await?;

        let mode_str = match mode {
            brassclaw_host_api::PermissionMode::Allow => "allow",
            brassclaw_host_api::PermissionMode::Ask => "ask",
            brassclaw_host_api::PermissionMode::Deny => "deny",
        };

        conn.execute(
            "INSERT INTO capability_permissions (tenant_id, capability_id, permission_mode, updated_at)
             VALUES ($1, $2, $3, NOW())
             ON CONFLICT (tenant_id, capability_id)
             DO UPDATE SET permission_mode = EXCLUDED.permission_mode, updated_at = NOW()",
            &[&tenant_id, &capability_id, &mode_str],
        )
        .await?;

        Ok(())
    }

    async fn delete_capability_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<bool, DatabaseError> {
        let conn = self.pool().get().await?;

        let rows_affected = conn
            .execute(
                "DELETE FROM capability_permissions WHERE tenant_id = $1 AND capability_id = $2",
                &[&tenant_id, &capability_id],
            )
            .await?;

        Ok(rows_affected > 0)
    }

    async fn list_capability_overrides(
        &self,
        tenant_id: &str,
    ) -> Result<std::collections::HashMap<String, brassclaw_host_api::PermissionMode>, DatabaseError>
    {
        let conn = self.pool().get().await?;

        let rows = conn
            .query(
                "SELECT capability_id, permission_mode FROM capability_permissions WHERE tenant_id = $1",
                &[&tenant_id],
            )
            .await?;

        let mut overrides = std::collections::HashMap::new();
        for row in rows {
            let capability_id: String = row.get(0);
            let mode_str: String = row.get(1);
            let mode = match mode_str.as_str() {
                "allow" => brassclaw_host_api::PermissionMode::Allow,
                "ask" => brassclaw_host_api::PermissionMode::Ask,
                "deny" => brassclaw_host_api::PermissionMode::Deny,
                _ => continue, // Skip invalid modes
            };
            overrides.insert(capability_id, mode);
        }

        Ok(overrides)
    }
}
