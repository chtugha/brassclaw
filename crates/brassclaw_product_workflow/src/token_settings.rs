//! Token settings types for WebUI v2.

use serde::{Deserialize, Serialize};

/// Response shape for token settings endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSettingsResponse {
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
}

/// Request body for updating token settings.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateTokenSettingsRequest {
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
}
