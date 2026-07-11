//! Token settings store trait for the WebUI v2 token limits endpoint.

use async_trait::async_trait;

use crate::token_settings::{TokenSettingsResponse, UpdateTokenSettingsRequest};

/// Port for reading and writing per-provider token limit settings.
///
/// Implementations bridge the product-workflow facade to the underlying
/// settings persistence layer (e.g. the main crate's `SettingsStore`).
#[async_trait]
pub trait TokenSettingsStore: Send + Sync {
    /// Load per-provider token settings for the given user + provider.
    async fn get_provider_token_settings(
        &self,
        user_id: &str,
        provider_id: &str,
    ) -> Result<TokenSettingsResponse, Box<dyn std::error::Error + Send + Sync>>;

    /// Persist per-provider token settings for the given user + provider.
    async fn update_provider_token_settings(
        &self,
        user_id: &str,
        provider_id: &str,
        request: UpdateTokenSettingsRequest,
    ) -> Result<TokenSettingsResponse, Box<dyn std::error::Error + Send + Sync>>;
}
