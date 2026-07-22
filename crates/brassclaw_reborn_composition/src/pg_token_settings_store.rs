//! Postgres-backed [`TokenSettingsStore`] implementation.
//!
//! Per-provider token limits are stored in `brassclaw_token_settings` (V014),
//! keyed by `(tenant_id, user_id, provider_id)`. The `settings` JSONB column
//! holds a `TokenSettingsResponse` snapshot.

// Phase-5 postgres wiring — items unused until factory wiring lands.
#![allow(dead_code)]

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{
    TokenSettingsResponse, TokenSettingsStore, UpdateTokenSettingsRequest,
};
use serde_json::Value;

fn map_pool(e: deadpool_postgres::PoolError) -> Box<dyn std::error::Error + Send + Sync> {
    e.to_string().into()
}

fn map_pg(e: tokio_postgres::Error) -> Box<dyn std::error::Error + Send + Sync> {
    e.to_string().into()
}

fn map_json(e: serde_json::Error) -> Box<dyn std::error::Error + Send + Sync> {
    e.to_string().into()
}

/// Postgres-backed [`TokenSettingsStore`].
pub(crate) struct PgTokenSettingsStore {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgTokenSettingsStore {
    pub(crate) fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }
}

#[async_trait]
impl TokenSettingsStore for PgTokenSettingsStore {
    async fn get_provider_token_settings(
        &self,
        user_id: &str,
        provider_id: &str,
    ) -> Result<TokenSettingsResponse, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "SELECT settings FROM brassclaw_token_settings \
                 WHERE tenant_id = $1 AND user_id = $2 AND provider_id = $3",
                &[&self.tenant_id, &user_id, &provider_id],
            )
            .await
            .map_err(map_pg)?;
        match row {
            None => Ok(empty_token_settings()),
            Some(r) => {
                let payload: Value = r.get(0);
                serde_json::from_value(payload).map_err(map_json)
            }
        }
    }

    async fn update_provider_token_settings(
        &self,
        user_id: &str,
        provider_id: &str,
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
            cache_retention: request.cache_retention,
        };
        let payload = serde_json::to_value(&response).map_err(map_json)?;
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "INSERT INTO brassclaw_token_settings \
                 (tenant_id, user_id, provider_id, settings) \
                 VALUES ($1, $2, $3, $4) \
                 ON CONFLICT (tenant_id, user_id, provider_id) \
                 DO UPDATE SET settings = excluded.settings",
                &[&self.tenant_id, &user_id, &provider_id, &payload],
            )
            .await
            .map_err(map_pg)?;
        Ok(response)
    }
}

fn empty_token_settings() -> TokenSettingsResponse {
    TokenSettingsResponse {
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
        cache_retention: None,
    }
}
