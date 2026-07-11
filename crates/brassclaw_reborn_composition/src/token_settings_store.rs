//! Token settings store implementation backed by the libSQL DB.
//!
//! Token limits are stored as a JSON object under the key `tokens`
//! in the settings table, keyed by user_id.  The settings table is
//! created here with `CREATE TABLE IF NOT EXISTS` if it does not yet exist
//! (for DBs that pre-date the settings migration).
//!
//! Per-provider limits are stored under the key `provider_tokens:<provider_id>`.

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_product_workflow::{
    TokenSettingsResponse, TokenSettingsStore, UpdateTokenSettingsRequest,
};
use tokio::sync::Mutex;

const TOKENS_SETTINGS_KEY: &str = "tokens";

fn provider_tokens_key(provider_id: &str) -> String {
    // provider_id is already validated as [a-z0-9_-]{1,64} at the API layer.
    format!("provider_tokens:{provider_id}")
}

const CREATE_SETTINGS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS settings (
    user_id    TEXT NOT NULL,
    key        TEXT NOT NULL,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (user_id, key)
);
CREATE INDEX IF NOT EXISTS idx_settings_user ON settings(user_id);
";

/// LibSQL-backed implementation of [`TokenSettingsStore`].
///
/// A single connection is kept open for the lifetime of the store.  This is
/// required for in-memory databases (used in tests) where each `db.connect()`
/// call would produce an independent, empty database.  For file-backed
/// databases the behaviour is identical; libsql connections on the same file
/// share the underlying WAL.
pub(crate) struct DbTokenSettingsStore {
    conn: Arc<Mutex<libsql::Connection>>,
}

impl DbTokenSettingsStore {
    /// Open the store, ensuring the `settings` table exists.
    pub(crate) async fn open(
        db: Arc<libsql::Database>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let conn = db
            .connect()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        conn.execute_batch(CREATE_SETTINGS_TABLE)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

#[async_trait]
impl TokenSettingsStore for DbTokenSettingsStore {
    async fn get_token_settings(
        &self,
        user_id: &str,
    ) -> Result<TokenSettingsResponse, Box<dyn std::error::Error + Send + Sync>> {
        let conn = self.conn.lock().await;
        let mut rows = conn
            .query(
                "SELECT value FROM settings WHERE user_id = ? AND key = ?",
                libsql::params![user_id, TOKENS_SETTINGS_KEY],
            )
            .await?;

        if let Some(row) = rows.next().await? {
            let value_str: String = row.get(0)?;
            let response: TokenSettingsResponse = serde_json::from_str(&value_str)?;
            Ok(response)
        } else {
            // Return empty (all None) when no settings exist yet.
            Ok(TokenSettingsResponse {
                profile: None,
                conversation_history: None,
                skills: None,
                identity: None,
                inline_control: None,
                memory: None,
                safety: None,
                capability_surface: None,
                total_input: None,
                max_output: None,
            })
        }
    }

    async fn update_token_settings(
        &self,
        user_id: &str,
        request: UpdateTokenSettingsRequest,
    ) -> Result<TokenSettingsResponse, Box<dyn std::error::Error + Send + Sync>> {
        let response = TokenSettingsResponse {
            profile: request.profile,
            conversation_history: request.conversation_history,
            skills: request.skills,
            identity: request.identity,
            inline_control: request.inline_control,
            memory: request.memory,
            safety: request.safety,
            capability_surface: request.capability_surface,
            total_input: request.total_input,
            max_output: request.max_output,
        };

        let value_str = serde_json::to_string(&response)?;
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO settings (user_id, key, value, updated_at) VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(user_id, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            libsql::params![user_id, TOKENS_SETTINGS_KEY, value_str],
        )
        .await?;

        Ok(response)
    }

    async fn get_provider_token_settings(
        &self,
        user_id: &str,
        provider_id: &str,
    ) -> Result<TokenSettingsResponse, Box<dyn std::error::Error + Send + Sync>> {
        let key = provider_tokens_key(provider_id);
        let conn = self.conn.lock().await;
        let mut rows = conn
            .query(
                "SELECT value FROM settings WHERE user_id = ? AND key = ?",
                libsql::params![user_id, key],
            )
            .await?;

        if let Some(row) = rows.next().await? {
            let value_str: String = row.get(0)?;
            let response: TokenSettingsResponse = serde_json::from_str(&value_str)?;
            Ok(response)
        } else {
            // Return empty (all None) when no per-provider settings exist yet.
            Ok(TokenSettingsResponse {
                profile: None,
                conversation_history: None,
                skills: None,
                identity: None,
                inline_control: None,
                memory: None,
                safety: None,
                capability_surface: None,
                total_input: None,
                max_output: None,
            })
        }
    }

    async fn update_provider_token_settings(
        &self,
        user_id: &str,
        provider_id: &str,
        request: UpdateTokenSettingsRequest,
    ) -> Result<TokenSettingsResponse, Box<dyn std::error::Error + Send + Sync>> {
        let key = provider_tokens_key(provider_id);
        let response = TokenSettingsResponse {
            profile: request.profile,
            conversation_history: request.conversation_history,
            skills: request.skills,
            identity: request.identity,
            inline_control: request.inline_control,
            memory: request.memory,
            safety: request.safety,
            capability_surface: request.capability_surface,
            total_input: request.total_input,
            max_output: request.max_output,
        };

        let value_str = serde_json::to_string(&response)?;
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO settings (user_id, key, value, updated_at) VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(user_id, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            libsql::params![user_id, key, value_str],
        )
        .await?;

        Ok(response)
    }
}

/// One-time forward migration: promote global token settings to the active
/// provider's per-provider key.
///
/// Runs non-destructively: if per-provider settings already exist for the
/// active provider, this is a no-op.  The global row is never deleted.
/// Call once at startup after the token store and active provider are both
/// available.
pub(crate) async fn migrate_global_tokens_to_active_provider(
    store: &DbTokenSettingsStore,
    active_provider_id: &str,
    user_id: &str,
) {
    // 1. Check if per-provider settings already exist.
    let existing = store
        .get_provider_token_settings(user_id, active_provider_id)
        .await;
    let has_existing = existing
        .as_ref()
        .map(|r| r.conversation_history.is_some() || r.profile.is_some())
        .unwrap_or(false);
    if has_existing {
        return; // already migrated; nothing to do
    }

    // 2. Read the global row.
    let Ok(global) = store.get_token_settings(user_id).await else {
        return;
    };

    // 3. If global has any non-None field, copy it to the per-provider key.
    if global.conversation_history.is_some() || global.profile.is_some() {
        let request = brassclaw_product_workflow::UpdateTokenSettingsRequest {
            profile: global.profile,
            conversation_history: global.conversation_history,
            skills: global.skills,
            identity: global.identity,
            inline_control: global.inline_control,
            memory: global.memory,
            safety: global.safety,
            capability_surface: global.capability_surface,
            total_input: global.total_input,
            max_output: global.max_output,
        };
        if let Err(e) = store
            .update_provider_token_settings(user_id, active_provider_id, request)
            .await
        {
            tracing::warn!(
                error = %e,
                provider_id = active_provider_id,
                "per-provider token settings migration failed; global defaults will apply"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_product_workflow::{TokenSettingsStore, UpdateTokenSettingsRequest};

    async fn in_memory_store() -> DbTokenSettingsStore {
        let db = Arc::new(
            libsql::Builder::new_local(":memory:")
                .build()
                .await
                .expect("in-memory libsql"),
        );
        DbTokenSettingsStore::open(db).await.expect("store open")
    }

    #[test]
    fn provider_tokens_key_format() {
        assert_eq!(provider_tokens_key("ollama"), "provider_tokens:ollama");
        assert_eq!(provider_tokens_key("my-provider"), "provider_tokens:my-provider");
    }

    #[tokio::test]
    async fn store_and_retrieve_provider_tokens() {
        let store = in_memory_store().await;
        let req = UpdateTokenSettingsRequest {
            profile: Some("small_7b".to_string()),
            conversation_history: Some(4000),
            skills: Some(3000),
            identity: None,
            inline_control: None,
            memory: None,
            safety: None,
            capability_surface: None,
            total_input: None,
            max_output: None,
        };
        store
            .update_provider_token_settings("user1", "ollama", req)
            .await
            .expect("update");

        let response = store
            .get_provider_token_settings("user1", "ollama")
            .await
            .expect("get");
        assert_eq!(response.profile, Some("small_7b".to_string()));
        assert_eq!(response.conversation_history, Some(4000));
        assert_eq!(response.skills, Some(3000));
        assert_eq!(response.identity, None);
    }

    #[tokio::test]
    async fn two_providers_are_isolated() {
        let store = in_memory_store().await;

        let req_a = UpdateTokenSettingsRequest {
            profile: None,
            conversation_history: Some(1000),
            skills: None,
            identity: None,
            inline_control: None,
            memory: None,
            safety: None,
            capability_surface: None,
            total_input: None,
            max_output: None,
        };
        let req_b = UpdateTokenSettingsRequest {
            profile: None,
            conversation_history: Some(2000),
            skills: None,
            identity: None,
            inline_control: None,
            memory: None,
            safety: None,
            capability_surface: None,
            total_input: None,
            max_output: None,
        };
        store
            .update_provider_token_settings("user1", "provider-a", req_a)
            .await
            .expect("update a");
        store
            .update_provider_token_settings("user1", "provider-b", req_b)
            .await
            .expect("update b");

        let a = store
            .get_provider_token_settings("user1", "provider-a")
            .await
            .expect("get a");
        let b = store
            .get_provider_token_settings("user1", "provider-b")
            .await
            .expect("get b");
        assert_eq!(a.conversation_history, Some(1000));
        assert_eq!(b.conversation_history, Some(2000));
    }

    #[tokio::test]
    async fn missing_provider_returns_all_none() {
        let store = in_memory_store().await;
        let response = store
            .get_provider_token_settings("user1", "never-written")
            .await
            .expect("get");
        assert_eq!(response.profile, None);
        assert_eq!(response.conversation_history, None);
        assert_eq!(response.skills, None);
    }
}
