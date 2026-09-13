//! Postgres-backed [`ConfigStore`] for the Agent and Networking settings tabs.
//!
//! Reads and writes the `brassclaw_config` table via [`crate::db_config`].
//! Only keys whose prefix appears in [`CONFIG_ALLOWED_PREFIXES`] are exposed —
//! no LLM credentials, interceptor tokens, or internal keys are ever returned.
//!
//! Feature-gated: `postgres`.

#[cfg(feature = "postgres")]
pub(crate) mod inner {
    use std::sync::Arc;

    use async_trait::async_trait;
    use brassclaw_pg::PgPool;
    use brassclaw_product_workflow::{
        CONFIG_ALLOWED_PREFIXES, ConfigStore, ConfigStoreError, SettingsConfigResponse,
        UpdateSettingResponse,
    };

    use crate::db_config::{self, ConfigWriteContext};

    /// Postgres-backed [`ConfigStore`] implementation.
    pub(crate) struct PgConfigStore {
        pool: Arc<PgPool>,
        tenant_id: String,
    }

    impl PgConfigStore {
        pub(crate) fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
            Self {
                pool,
                tenant_id: tenant_id.into(),
            }
        }
    }

    /// Return `true` when `key` starts with one of the allowed prefixes.
    fn is_key_allowed(key: &str) -> bool {
        CONFIG_ALLOWED_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
    }

    #[async_trait]
    impl ConfigStore for PgConfigStore {
        async fn get_all(&self) -> Result<SettingsConfigResponse, ConfigStoreError> {
            let rows = db_config::list_config_keys(&self.pool, &self.tenant_id)
                .await
                .map_err(|e| ConfigStoreError::Unavailable(e.to_string()))?;

            let settings = rows
                .into_iter()
                .filter(|(key, _)| is_key_allowed(key))
                .collect();

            Ok(SettingsConfigResponse { settings })
        }

        async fn set_key(
            &self,
            key: &str,
            value: &str,
        ) -> Result<UpdateSettingResponse, ConfigStoreError> {
            if !is_key_allowed(key) {
                return Err(ConfigStoreError::KeyNotAllowed {
                    key: key.to_string(),
                });
            }

            if value.is_empty() {
                // Empty value = delete the key.
                db_config::delete_config_key(&self.pool, &self.tenant_id, key)
                    .await
                    .map_err(|e| ConfigStoreError::QueryFailed(e.to_string()))?;
                return Ok(UpdateSettingResponse {
                    key: key.to_string(),
                    value: None,
                    success: true,
                });
            }

            db_config::save_config_key(
                &self.pool,
                &self.tenant_id,
                key,
                value,
                ConfigWriteContext::Operator,
            )
            .await
            .map_err(|e| ConfigStoreError::QueryFailed(e.to_string()))?;

            Ok(UpdateSettingResponse {
                key: key.to_string(),
                value: Some(value.to_string()),
                success: true,
            })
        }
    }
}

#[cfg(feature = "postgres")]
pub(crate) use inner::PgConfigStore;
