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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct InMemoryTokenSettingsStore {
        data: Mutex<HashMap<String, TokenSettingsResponse>>,
    }

    impl InMemoryTokenSettingsStore {
        fn new() -> Self {
            Self {
                data: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl TokenSettingsStore for InMemoryTokenSettingsStore {
        async fn get_provider_token_settings(
            &self,
            user_id: &str,
            provider_id: &str,
        ) -> Result<TokenSettingsResponse, Box<dyn std::error::Error + Send + Sync>> {
            let key = format!("{user_id}:{provider_id}");
            let data = self.data.lock().map_err(|e| e.to_string())?;
            Ok(data.get(&key).cloned().unwrap_or(TokenSettingsResponse {
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
            }))
        }

        async fn update_provider_token_settings(
            &self,
            user_id: &str,
            provider_id: &str,
            request: UpdateTokenSettingsRequest,
        ) -> Result<TokenSettingsResponse, Box<dyn std::error::Error + Send + Sync>> {
            let key = format!("{user_id}:{provider_id}");
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
            self.data
                .lock()
                .map_err(|e| e.to_string())?
                .insert(key, response.clone());
            Ok(response)
        }
    }

    #[tokio::test]
    async fn token_settings_round_trip() {
        let store = InMemoryTokenSettingsStore::new();
        let user = "test-user";
        let provider = "test-provider";

        let initial = store
            .get_provider_token_settings(user, provider)
            .await
            .expect("get must succeed");
        assert!(initial.conversation_history.is_none());

        let request = UpdateTokenSettingsRequest {
            profile: Some("small_7b".to_string()),
            conversation_history: Some(4000),
            skills: Some(3000),
            identity: Some(2000),
            inline_control: Some(500),
            memory: Some(500),
            safety: Some(100),
            capability_surface: Some(1500),
            total_input: Some(12000),
            max_output: Some(2048),
            cache_retention: Some("short".to_string()),
        };
        let updated = store
            .update_provider_token_settings(user, provider, request)
            .await
            .expect("update must succeed");
        assert_eq!(updated.conversation_history, Some(4000));
        assert_eq!(updated.max_output, Some(2048));
        assert_eq!(updated.profile.as_deref(), Some("small_7b"));

        let read_back = store
            .get_provider_token_settings(user, provider)
            .await
            .expect("get must succeed");
        assert_eq!(read_back.conversation_history, updated.conversation_history);
        assert_eq!(read_back.skills, updated.skills);
        assert_eq!(read_back.identity, updated.identity);
        assert_eq!(read_back.inline_control, updated.inline_control);
        assert_eq!(read_back.memory, updated.memory);
        assert_eq!(read_back.safety, updated.safety);
        assert_eq!(read_back.capability_surface, updated.capability_surface);
        assert_eq!(read_back.total_input, updated.total_input);
        assert_eq!(read_back.max_output, updated.max_output);
        assert_eq!(read_back.profile, updated.profile);
    }

    #[tokio::test]
    async fn token_settings_update_overwrites_previous() {
        let store = InMemoryTokenSettingsStore::new();
        let user = "test-user";
        let provider = "groq";

        let req1 = UpdateTokenSettingsRequest {
            profile: None,
            conversation_history: Some(8000),
            skills: None,
            identity: None,
            inline_control: None,
            memory: None,
            safety: None,
            capability_surface: None,
            total_input: None,
            max_output: Some(4096),
            cache_retention: Some("long".to_string()),
        };
        store
            .update_provider_token_settings(user, provider, req1)
            .await
            .expect("first update");

        let req2 = UpdateTokenSettingsRequest {
            profile: None,
            conversation_history: Some(2000),
            skills: None,
            identity: None,
            inline_control: None,
            memory: None,
            safety: None,
            capability_surface: None,
            total_input: None,
            max_output: Some(1024),
            cache_retention: Some("none".to_string()),
        };
        let updated = store
            .update_provider_token_settings(user, provider, req2)
            .await
            .expect("second update");

        assert_eq!(updated.conversation_history, Some(2000));
        assert_eq!(updated.max_output, Some(1024));

        let read = store
            .get_provider_token_settings(user, provider)
            .await
            .expect("read after overwrite");
        assert_eq!(read.conversation_history, Some(2000));
        assert_eq!(read.max_output, Some(1024));
    }
}
