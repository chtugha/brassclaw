//! Token settings types for WebUI v2.

use serde::{Deserialize, Serialize};

/// Response shape for token settings endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSettingsResponse {
    /// Named distribution preset (`small_7b`, `large`, `coding`, `chat`).
    /// `null` means no preset — all fields are individually configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Max tokens for conversation/thread history messages.
    pub conversation_history: Option<usize>,
    /// Max tokens for skill/instruction snippets.
    pub skills: Option<usize>,
    /// Max tokens for identity/persona messages.
    pub identity: Option<usize>,
    /// Max tokens for inline control messages (loop nudges).
    pub inline_control: Option<usize>,
    /// Max tokens for memory snippets.
    pub memory: Option<usize>,
    /// Max tokens for safety context.
    pub safety: Option<usize>,
    /// Max tokens for visible capability surface (tool descriptions).
    pub capability_surface: Option<usize>,
    /// Max tokens for total input (across all sections).
    pub total_input: Option<usize>,
    /// Max output tokens requested from the model.
    pub max_output: Option<usize>,
    /// Prompt cache retention policy for Anthropic-compatible providers.
    /// Valid values: `"none"`, `"short"`, `"long"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_retention: Option<String>,
}

/// Request body for updating token settings.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateTokenSettingsRequest {
    /// Named distribution preset (`small_7b`, `large`, `coding`, `chat`).
    /// `null` clears the preset selection (switches to Custom).
    #[serde(default)]
    pub profile: Option<String>,
    /// Max tokens for conversation/thread history messages.
    pub conversation_history: Option<usize>,
    /// Max tokens for skill/instruction snippets.
    pub skills: Option<usize>,
    /// Max tokens for identity/persona messages.
    pub identity: Option<usize>,
    /// Max tokens for inline control messages (loop nudges).
    pub inline_control: Option<usize>,
    /// Max tokens for memory snippets.
    pub memory: Option<usize>,
    /// Max tokens for safety context.
    pub safety: Option<usize>,
    /// Max tokens for visible capability surface (tool descriptions).
    pub capability_surface: Option<usize>,
    /// Max tokens for total input (across all sections).
    pub total_input: Option<usize>,
    /// Max output tokens requested from the model.
    pub max_output: Option<usize>,
    /// Prompt cache retention policy. Valid values: `"none"`, `"short"`, `"long"`.
    #[serde(default)]
    pub cache_retention: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `cache_retention` must round-trip through JSON cleanly so the WebUI
    /// PUT → GET preview behaviour matches storage. Regression guard for
    /// the field being added without `#[serde(default)]` on the request
    /// path — missing defaults would reject PUT requests that omit the
    /// field, breaking existing clients.
    #[test]
    fn update_request_accepts_missing_cache_retention() {
        let json = r#"{"conversation_history": 1024}"#;
        let req: UpdateTokenSettingsRequest =
            serde_json::from_str(json).expect("missing cache_retention must default");
        assert!(req.cache_retention.is_none());
    }

    #[test]
    fn update_request_accepts_explicit_cache_retention() {
        let json = r#"{"cache_retention": "short"}"#;
        let req: UpdateTokenSettingsRequest =
            serde_json::from_str(json).expect("explicit cache_retention must parse");
        assert_eq!(req.cache_retention.as_deref(), Some("short"));
    }

    #[test]
    fn response_round_trips_cache_retention() {
        let json = r#"{"cache_retention": "long"}"#;
        let resp: TokenSettingsResponse =
            serde_json::from_str(json).expect("response parse must succeed");
        assert_eq!(resp.cache_retention.as_deref(), Some("long"));

        let serialised = serde_json::to_string(&resp).expect("response serialise must succeed");
        let reparsed: TokenSettingsResponse =
            serde_json::from_str(&serialised).expect("response re-parse must succeed");
        assert_eq!(reparsed.cache_retention.as_deref(), Some("long"));
    }
}
