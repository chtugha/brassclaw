//! Token settings store implementation backed by the libSQL DB.
//!
//! Token limits are stored as a JSON object under the key `tokens`
//! in the settings table, keyed by user_id.  The settings table is
//! created here with `CREATE TABLE IF NOT EXISTS` if it does not yet exist
//! (for DBs that pre-date the settings migration).

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_product_workflow::{
    TokenSettingsResponse, TokenSettingsStore, UpdateTokenSettingsRequest,
};

const TOKENS_SETTINGS_KEY: &str = "tokens";

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
pub(crate) struct DbTokenSettingsStore {
    db: Arc<libsql::Database>,
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
        Ok(Self { db })
    }

    async fn connection(&self) -> Result<libsql::Connection, Box<dyn std::error::Error + Send + Sync>> {
        self.db
            .connect()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

#[async_trait]
impl TokenSettingsStore for DbTokenSettingsStore {
    async fn get_token_settings(
        &self,
        user_id: &str,
    ) -> Result<TokenSettingsResponse, Box<dyn std::error::Error + Send + Sync>> {
        let conn = self.connection().await?;
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
        let conn = self.connection().await?;
        conn.execute(
            "INSERT INTO settings (user_id, key, value, updated_at) VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(user_id, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            libsql::params![user_id, TOKENS_SETTINGS_KEY, value_str],
        )
        .await?;

        Ok(response)
    }
}
