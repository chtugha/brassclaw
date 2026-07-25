//! `PgUserPreferenceStore` — Postgres-backed per-user chat preferences.
//!
//! Reads and writes the `reborn_user_preferences` table (V035 migration).
//! Scoped by `user_id` only (not full scope tuple — preferences are per-user
//! across all agents/projects per spec §7 Q18).

#[cfg(feature = "postgres")]
mod inner {
    use std::sync::Arc;

    use async_trait::async_trait;
    use brassclaw_pg::PgPool;
    use tracing::debug;

    /// Allowed preference keys.  Any key not in this list is rejected.
    const ALLOWED_KEYS: &[&str] = &["ai_before_user"];

    #[derive(Debug, thiserror::Error)]
    pub(crate) enum PgUserPreferenceError {
        #[error("pool error: {0}")]
        Pool(String),
        #[error("internal error: {0}")]
        Internal(String),
        #[error("unknown preference key: {0}")]
        UnknownKey(String),
    }

    /// Postgres-backed user preference store.
    #[derive(Clone)]
    pub(crate) struct PgUserPreferenceStore {
        pool: Arc<PgPool>,
    }

    impl PgUserPreferenceStore {
        pub(crate) fn new(pool: Arc<PgPool>) -> Self {
            Self { pool }
        }

        /// Upsert a preference value.  Returns the stored JSON value.
        pub(crate) async fn upsert(
            &self,
            user_id: &str,
            key: &str,
            value: &serde_json::Value,
        ) -> Result<serde_json::Value, PgUserPreferenceError> {
            if !ALLOWED_KEYS.contains(&key) {
                return Err(PgUserPreferenceError::UnknownKey(key.to_string()));
            }

            let value_str = value.to_string();
            let client = self
                .pool
                .get()
                .await
                .map_err(|e| PgUserPreferenceError::Pool(e.to_string()))?;
            client
                .execute(
                    "INSERT INTO reborn_user_preferences
                         (user_id, preference_key, preference_value)
                     VALUES ($1, $2, $3)
                     ON CONFLICT ON CONSTRAINT reborn_user_preferences_user_key_unique
                     DO UPDATE SET
                         preference_value = EXCLUDED.preference_value,
                         updated_at       = now()",
                    &[&user_id, &key, &value_str],
                )
                .await
                .map_err(|e| PgUserPreferenceError::Internal(e.to_string()))?;
            debug!(user_id, key, "user_preference: upserted");
            serde_json::from_str(&value_str)
                .map_err(|e| PgUserPreferenceError::Internal(e.to_string()))
        }
    }

    #[async_trait]
    impl brassclaw_product_workflow::ChatPreferenceStore for PgUserPreferenceStore {
        async fn upsert(
            &self,
            user_id: &str,
            key: &str,
            value: &serde_json::Value,
        ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
            PgUserPreferenceStore::upsert(self, user_id, key, value)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }
}

#[cfg(feature = "postgres")]
pub(crate) use inner::PgUserPreferenceStore;
