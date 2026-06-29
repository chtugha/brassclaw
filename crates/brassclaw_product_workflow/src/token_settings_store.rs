//! Token settings store trait for the WebUI v2 token limits endpoint.

use async_trait::async_trait;

use crate::token_settings::{TokenSettingsResponse, UpdateTokenSettingsRequest};

/// Port for reading and writing token limit settings.
///
/// Implementations bridge the product-workflow facade to the underlying
/// settings persistence layer (e.g. the main crate's `SettingsStore`).
#[async_trait]
pub trait TokenSettingsStore: Send + Sync {
    /// Load the current token settings for the given user.
    async fn get_token_settings(
        &self,
        user_id: &str,
    ) -> Result<TokenSettingsResponse, Box<dyn std::error::Error + Send + Sync>>;

    /// Persist updated token settings for the given user.
    async fn update_token_settings(
        &self,
        user_id: &str,
        request: UpdateTokenSettingsRequest,
    ) -> Result<TokenSettingsResponse, Box<dyn std::error::Error + Send + Sync>>;
}
