use crate::error::ConfigError;
use crate::settings::Settings;

/// Resolved token limit configuration for LLM context composition.
///
/// All limits are optional — `None` means "use the runtime compiled default".
#[derive(Debug, Clone, Default)]
pub struct TokensConfig {
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

    /// Max tokens for the visible capability surface (tool descriptions).
    pub capability_surface: Option<usize>,

    /// Max tokens for total input (across all sections).
    pub total_input: Option<usize>,

    /// Max output tokens requested from the model.
    pub max_output: Option<usize>,
}

impl TokensConfig {
    pub(crate) fn resolve(settings: &Settings) -> Result<Self, ConfigError> {
        let ts = &settings.tokens;
        Ok(Self {
            conversation_history: ts.conversation_history,
            skills: ts.skills,
            identity: ts.identity,
            inline_control: ts.inline_control,
            memory: ts.memory,
            safety: ts.safety,
            capability_surface: ts.capability_surface,
            total_input: ts.total_input,
            max_output: ts.max_output,
        })
    }
}
